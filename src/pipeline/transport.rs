//! Node-to-node hidden-state transport for the shard chain.
//!
//! This is the missing primitive of the whole project: today everything is star topology
//! (node ↔ router); pipeline serving needs a node ↔ node data link. The residual-stream hidden
//! state crosses each shard boundary here, over a quinn bi-stream — the same QUIC stack the node
//! already uses for the router, with dkn-protocol's length-prefixed msgpack framing
//! ([`read_framed`]/[`write_framed`]).
//!
//! This module is the WIRE. [`super::qwen::shard::ShardedChain`] does the in-process hand-off; a
//! remote shard replaces that one `Vec<f32>` hop with [`PeerConnection::send`] of a
//! [`HiddenStateFrame`]. The hidden round-trip is lossless (f32 bytes → msgpack bin → f32).
//!
//! STATUS: frames + a working loopback `PeerConnection` (insecure TLS, like `--insecure` to the
//! router). NAT traversal / router-brokered path formation and the activation codec are later
//! (see `docs/chain-formation-design.md`, `docs/pipeline-specdecode-design.md`).
#![allow(dead_code)]

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use quinn::{ClientConfig, Endpoint, RecvStream, SendStream, ServerConfig, TransportConfig};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::DigitallySignedStruct;
use serde::{Deserialize, Serialize};

use super::engine::TokenId;
use crate::network::protocol::{read_framed, write_framed};

/// ALPN for node↔node shard streams (distinct from the router's `dkn`).
const ALPN: &[u8] = b"dkn-shard";

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

/// A residual-stream hidden state crossing a shard boundary: logically `[n_tokens, hidden]`.
/// Bandwidth is small (~12 KiB/token at H=6144, bf16) — the wire is RTT-bound, not byte-bound —
/// so the activation codec (`dtype`) is a second-order optimization for thin home uplinks, not
/// the latency lever. The payload is the row-major hidden state in little-endian `dtype`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HiddenStateFrame {
    pub request_id: u64,
    pub chain_id: u64,
    /// Bumped on any chain re-plan; a node drops frames with a stale epoch (fencing).
    pub epoch: u64,
    pub step: u64,
    /// Absolute sequence position of the first token in this chunk (the spec-decode `start_pos`);
    /// every shard writes its KV at the same absolute positions, which makes rollback a no-op.
    pub start_pos: u32,
    /// Which layer-block boundary this hidden state sits at (the cut index).
    pub layer_boundary: u32,
    pub n_tokens: u32,
    pub hidden_dim: u32,
    /// Activation codec on the wire, e.g. "f32" | "bf16" | "fp8".
    pub dtype: String,
    pub data: Vec<u8>,
}

impl HiddenStateFrame {
    /// Build an f32 hidden-state frame from a flat `[n_tokens * hidden_dim]` residual stream.
    pub fn from_f32(
        request_id: u64,
        epoch: u64,
        step: u64,
        start_pos: u32,
        layer_boundary: u32,
        n_tokens: u32,
        hidden_dim: u32,
        hidden: &[f32],
    ) -> Self {
        Self {
            request_id,
            chain_id: 0,
            epoch,
            step,
            start_pos,
            layer_boundary,
            n_tokens,
            hidden_dim,
            dtype: "f32".to_string(),
            data: f32_to_le_bytes(hidden),
        }
    }

    /// Decode the payload back to a flat f32 residual stream (only `dtype == "f32"` for now).
    pub fn to_f32(&self) -> Vec<f32> {
        debug_assert_eq!(self.dtype, "f32", "only the f32 codec is implemented");
        le_bytes_to_f32(&self.data)
    }
}

/// The tail returns this to node-0 (direct return, one hop) for spec-decode verification: the
/// target's greedy argmax at each of the `gamma + 1` verify positions. (node-0 owns `lm_head` +
/// the draft, so acceptance logits stay numerically consistent with the draft; for true sampling
/// later this carries logprobs instead — see design docs.)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerifyResult {
    pub request_id: u64,
    pub step: u64,
    pub argmax: Vec<TokenId>,
}

/// Roll back every shard's KV to the committed prefix. With absolute-position KV writes this is
/// advisory (the next chunk overwrites the rejected tail); maps to [`super::engine::TargetChain::truncate`].
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct KvTruncate {
    pub request_id: u64,
    pub epoch: u64,
    pub keep_to_pos: u32,
}

/// One framed message on a shard bi-stream.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PeerMessage {
    Hidden(HiddenStateFrame),
    Result(VerifyResult),
    Truncate(KvTruncate),
}

fn f32_to_le_bytes(xs: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(xs.len() * 4);
    for &x in xs {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

fn le_bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

// ---------------------------------------------------------------------------
// Duplex link to one adjacent peer
// ---------------------------------------------------------------------------

/// A duplex link to one adjacent shard (next/prev hop), or tail→owner direct return: the two
/// halves of a single quinn bi-stream, with one in-flight (request, hop). Epoch fencing and
/// app-level backpressure layer on top when the chain control-plane lands.
pub struct PeerConnection {
    // Held to keep the QUIC connection open: dropping the last Connection handle closes it,
    // even while the stream halves are still alive.
    conn: quinn::Connection,
    send: SendStream,
    recv: RecvStream,
}

impl PeerConnection {
    pub fn new(conn: quinn::Connection, send: SendStream, recv: RecvStream) -> Self {
        Self { conn, send, recv }
    }

    pub async fn send(&mut self, msg: &PeerMessage) -> io::Result<()> {
        write_framed(&mut self.send, msg).await.map_err(io_other)
    }

    /// Read the next message; `Ok(None)` on clean stream EOF.
    pub async fn recv(&mut self) -> io::Result<Option<PeerMessage>> {
        read_framed(&mut self.recv).await.map_err(io_other)
    }

    /// Close this side of the connection (call after the final frame is read). Dropping the
    /// connection sends an immediate CONNECTION_CLOSE that can discard un-read data, so the
    /// reader closes explicitly and the writer awaits [`wait_closed`] before tearing down.
    pub fn close(&self) {
        self.conn.close(0u32.into(), b"bye");
    }

    /// Await the peer closing the connection — the writer of the last frame awaits this so it
    /// does not drop (and truncate) before the reader has the frame.
    pub async fn wait_closed(&self) {
        self.conn.closed().await;
    }

    /// Finish the send half cleanly (FIN). The peer's next `recv` returns `Ok(None)` (clean EOF)
    /// rather than a "connection lost" error — used to end a session gracefully.
    pub fn finish(&mut self) {
        let _ = self.send.finish();
    }
}

// ---------------------------------------------------------------------------
// Endpoints (insecure TLS — dev/test + `--insecure`; prod pins peer certs at path formation)
// ---------------------------------------------------------------------------

/// A client endpoint that dials shard peers (accepts any server cert).
pub fn dial_endpoint() -> io::Result<Endpoint> {
    let mut crypto = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
        .with_no_client_auth();
    crypto.alpn_protocols = vec![ALPN.to_vec()];
    let mut client_config = ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(crypto).map_err(io_other)?,
    ));
    client_config.transport_config(Arc::new(keepalive_transport()));
    let mut ep = Endpoint::client("0.0.0.0:0".parse().unwrap()).map_err(io_other)?;
    ep.set_default_client_config(client_config);
    Ok(ep)
}

/// A listening endpoint for an inbound shard peer (self-signed cert; allows client-opened
/// bi-streams so the upstream node initiates each hop).
pub fn listen_endpoint(addr: SocketAddr) -> io::Result<Endpoint> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).map_err(io_other)?;
    let cert_der = CertificateDer::from(cert.cert);
    let key_der = PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());

    let mut server_crypto = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der.into())
        .map_err(io_other)?;
    server_crypto.alpn_protocols = vec![ALPN.to_vec()];

    let mut server_config = ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto).map_err(io_other)?,
    ));
    let mut transport = keepalive_transport();
    transport.max_concurrent_bidi_streams(8u32.into());
    server_config.transport_config(Arc::new(transport));

    Endpoint::server(server_config, addr).map_err(io_other)
}

/// Transport config with keepalives so a persistent shard link survives long idle gaps (e.g. a
/// multi-minute model load before the first hop) without hitting the idle timeout.
fn keepalive_transport() -> TransportConfig {
    let mut transport = TransportConfig::default();
    transport.keep_alive_interval(Some(Duration::from_secs(5)));
    transport.max_idle_timeout(Some(Duration::from_secs(30).try_into().expect("valid idle timeout")));
    transport
}

/// Dial `addr` and open one bi-stream for a hop (upstream node → this peer).
pub async fn dial(ep: &Endpoint, addr: SocketAddr) -> io::Result<PeerConnection> {
    let conn = ep
        .connect(addr, "localhost")
        .map_err(io_other)?
        .await
        .map_err(io_other)?;
    let (send, recv) = conn.open_bi().await.map_err(io_other)?;
    Ok(PeerConnection::new(conn, send, recv))
}

/// Accept one inbound connection + its first bi-stream (resolves once the dialer sends).
pub async fn accept(ep: &Endpoint) -> io::Result<PeerConnection> {
    let incoming = ep.accept().await.ok_or_else(|| io_other("endpoint closed"))?;
    let conn = incoming.await.map_err(io_other)?;
    let (send, recv) = conn.accept_bi().await.map_err(io_other)?;
    Ok(PeerConnection::new(conn, send, recv))
}

fn io_other<E: ToString>(e: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e.to_string())
}

/// TLS verifier that accepts any certificate (dev/test peer links; prod pins certs).
#[derive(Debug)]
struct SkipServerVerification;

impl ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The node→node hop: a hidden state crosses a real QUIC link and the tail returns a result.
    // Proves the wire (serialization + framing + bi-stream) end to end, GPU-free.
    #[tokio::test]
    async fn hidden_state_roundtrips_over_quic() {
        let server = listen_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
        let addr = server.local_addr().unwrap();

        // A residual stream with negatives + fractions to catch any byte/codec corruption.
        let hidden: Vec<f32> = (0..32).map(|i| (i as f32) * 0.37 - 5.5).collect();
        let hidden_expect = hidden.clone();

        // Tail peer: receive the hidden state, "run its shard", return a VerifyResult.
        let server_fut = async move {
            let mut peer = accept(&server).await?;
            let frame = match peer.recv().await?.expect("clean EOF before a frame") {
                PeerMessage::Hidden(f) => f,
                other => panic!("expected Hidden, got {other:?}"),
            };
            let got = frame.to_f32();
            // Stand-in for the last shard's argmax: report what we received so the client can check.
            let argmax = vec![got.len() as TokenId, frame.n_tokens as TokenId, frame.start_pos as TokenId];
            peer.send(&PeerMessage::Result(VerifyResult {
                request_id: frame.request_id,
                step: frame.step,
                argmax,
            }))
            .await?;
            peer.wait_closed().await; // hold the connection open until the client has the result
            io::Result::Ok(got)
        };

        // Owner peer (node-0): dial, send the hidden state, read the result.
        let client = dial_endpoint().unwrap();
        let client_fut = async {
            let mut peer = dial(&client, addr).await?;
            let frame = HiddenStateFrame::from_f32(42, 1, 0, 7, 14, 4, 8, &hidden);
            peer.send(&PeerMessage::Hidden(frame)).await?;
            let result = match peer.recv().await?.expect("clean EOF before a result") {
                PeerMessage::Result(r) => r,
                other => panic!("expected Result, got {other:?}"),
            };
            peer.close(); // release the server's wait_closed
            io::Result::Ok(result)
        };

        let (server_got, result) = tokio::join!(server_fut, client_fut);
        let server_got = server_got.expect("server side");
        let result = result.expect("client side");
        assert_eq!(server_got, hidden_expect, "hidden state must survive the QUIC round-trip exactly");
        assert_eq!(result.request_id, 42);
        assert_eq!(result.argmax, vec![32, 4, 7], "n_floats=32, n_tokens=4, start_pos=7");
    }
}
