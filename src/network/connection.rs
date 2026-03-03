use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use quinn::{ClientConfig, Endpoint, TransportConfig};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::DigitallySignedStruct;

use crate::error::NodeError;
use crate::identity::Identity;
use crate::network::auth::authenticate;
use crate::network::protocol::{Capacity, NodeMessage, RouterMessage, read_framed, write_framed};

/// Manages a QUIC connection to the router with a single bi-directional stream.
pub struct RouterConnection {
    /// The underlying QUIC endpoint (kept alive for the connection's lifetime).
    endpoint: Endpoint,
    /// The underlying QUIC connection.
    connection: quinn::Connection,
    /// Send half of the bi-directional stream.
    send: quinn::SendStream,
    /// Receive half of the bi-directional stream.
    recv: quinn::RecvStream,
    /// Assigned node ID from the router.
    pub node_id: String,
}

impl RouterConnection {
    /// Establish a QUIC connection to the router, open a bi-stream, and authenticate.
    pub async fn connect(
        router_url: &str,
        insecure: bool,
        identity: &Identity,
        models: Vec<String>,
        tps: HashMap<String, f64>,
        capacity: Capacity,
    ) -> Result<Self, NodeError> {
        let (host, port) = parse_url(router_url)?;
        let addr = resolve_addr(&host, port).await?;

        let client_config = build_client_config(insecure)?;
        let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap())
            .map_err(|e| NodeError::Network(format!("failed to create QUIC endpoint: {e}")))?;
        endpoint.set_default_client_config(client_config);

        let connection = endpoint
            .connect(addr, &host)
            .map_err(|e| NodeError::Network(format!("QUIC connect failed: {e}")))?
            .await
            .map_err(|e| NodeError::Network(format!("QUIC handshake failed: {e}")))?;

        // Accept the bi-stream opened by the router (the router initiates the stream).
        let (mut send, mut recv) = connection
            .accept_bi()
            .await
            .map_err(|e| NodeError::Network(format!("failed to accept bi-stream: {e}")))?;
        let node_id = authenticate(&mut send, &mut recv, identity, models, tps, capacity).await?;
        tracing::info!(%node_id, "authenticated with router");

        Ok(RouterConnection {
            endpoint,
            connection,
            send,
            recv,
            node_id,
        })
    }

    /// Send a message to the router.
    pub async fn send(&mut self, msg: &NodeMessage) -> Result<(), NodeError> {
        Ok(write_framed(&mut self.send, msg).await?)
    }

    /// Receive a message from the router. Returns `None` on clean stream close.
    pub async fn recv(&mut self) -> Result<Option<RouterMessage>, NodeError> {
        Ok(read_framed(&mut self.recv).await?)
    }

    /// Close the connection and endpoint gracefully.
    pub fn close(&self) {
        self.connection.close(0u32.into(), b"shutdown");
        self.endpoint.close(0u32.into(), b"shutdown");
    }
}

// ---------------------------------------------------------------------------
// TLS configuration
// ---------------------------------------------------------------------------

fn build_client_config(insecure: bool) -> Result<ClientConfig, NodeError> {
    let mut crypto = if insecure {
        rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
            .with_no_client_auth()
    } else {
        let mut root_store = rustls::RootCertStore::empty();
        for cert in rustls_native_certs::load_native_certs().certs {
            root_store.add(cert).ok();
        }
        rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth()
    };
    crypto.alpn_protocols = vec![b"dkn".to_vec()];

    let mut transport = TransportConfig::default();
    transport.keep_alive_interval(Some(Duration::from_secs(20)));
    transport.max_idle_timeout(Some(
        Duration::from_secs(60)
            .try_into()
            .map_err(|e| NodeError::Network(format!("invalid idle timeout: {e}")))?,
    ));

    let mut client_config = ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(crypto)
            .map_err(|e| NodeError::Network(format!("QUIC crypto config: {e}")))?,
    ));
    client_config.transport_config(Arc::new(transport));

    Ok(client_config)
}

/// TLS verifier that accepts any certificate (for development/testing with `--insecure`).
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

// ---------------------------------------------------------------------------
// URL parsing and DNS resolution
// ---------------------------------------------------------------------------

fn parse_url(url: &str) -> Result<(String, u16), NodeError> {
    // Support both "host:port" and "https://host:port" formats
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("quic://"))
        .unwrap_or(url);

    let (host, port) = if let Some((h, p)) = stripped.rsplit_once(':') {
        let port: u16 = p
            .parse()
            .map_err(|_| NodeError::Network(format!("invalid port in URL: {url}")))?;
        (h.to_string(), port)
    } else {
        (stripped.to_string(), 4001) // default QUIC port
    };

    Ok((host, port))
}

async fn resolve_addr(host: &str, port: u16) -> Result<SocketAddr, NodeError> {
    // Try parsing as IP address first
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return Ok(SocketAddr::new(ip, port));
    }

    // DNS resolution
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host(format!("{host}:{port}"))
        .await
        .map_err(|e| NodeError::Network(format!("DNS resolution failed for {host}: {e}")))?
        .collect();

    addrs
        .into_iter()
        .next()
        .ok_or_else(|| NodeError::Network(format!("no addresses found for {host}")))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_url_with_scheme() {
        let (host, port) = parse_url("https://router.dria.co:4001").unwrap();
        assert_eq!(host, "router.dria.co");
        assert_eq!(port, 4001);
    }

    #[test]
    fn test_parse_url_quic_scheme() {
        let (host, port) = parse_url("quic://router.dria.co:5000").unwrap();
        assert_eq!(host, "router.dria.co");
        assert_eq!(port, 5000);
    }

    #[test]
    fn test_parse_url_no_scheme() {
        let (host, port) = parse_url("router.dria.co:4001").unwrap();
        assert_eq!(host, "router.dria.co");
        assert_eq!(port, 4001);
    }

    #[test]
    fn test_parse_url_default_port() {
        let (host, port) = parse_url("https://router.dria.co").unwrap();
        assert_eq!(host, "router.dria.co");
        assert_eq!(port, 4001);
    }

    #[test]
    fn test_parse_url_ip_address() {
        let (host, port) = parse_url("127.0.0.1:4001").unwrap();
        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 4001);
    }

    #[test]
    fn test_build_client_config_insecure() {
        let config = build_client_config(true);
        assert!(config.is_ok());
    }

    #[test]
    fn test_build_client_config_secure() {
        let config = build_client_config(false);
        assert!(config.is_ok());
    }

    /// Integration test: QUIC raw stream exchange with local self-signed server.
    /// Tests the full flow: connect, open stream, exchange framed messages.
    #[tokio::test]
    async fn test_quic_connection_with_local_server() {
        tokio::time::timeout(Duration::from_secs(10), async {
            // Generate self-signed cert
            let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
            let cert_der = CertificateDer::from(cert.cert);
            let key_der =
                rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());

            // Build server config
            let mut server_crypto = rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(vec![cert_der.clone()], key_der.into())
                .unwrap();
            server_crypto.alpn_protocols = vec![b"dkn".to_vec()];

            let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(
                quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto).unwrap(),
            ));
            let mut transport = TransportConfig::default();
            transport.max_concurrent_bidi_streams(8u32.into());
            server_config.transport_config(Arc::new(transport));

            // Bind server
            let server_endpoint =
                Endpoint::server(server_config, "127.0.0.1:0".parse().unwrap()).unwrap();
            let server_addr = server_endpoint.local_addr().unwrap();

            // Use a oneshot to signal server completion
            let (tx, rx) = tokio::sync::oneshot::channel::<()>();

            // Spawn server task — the server opens the bi-stream (router initiates)
            tokio::spawn(async move {
                let incoming = server_endpoint.accept().await.unwrap();
                let server_conn = incoming.await.unwrap();

                // Server opens a bi-stream to the client
                let (mut send, mut recv) = server_conn.open_bi().await.unwrap();

                // Send challenge
                let challenge = crate::network::auth::ChallengeMessage {
                    challenge: [0xAA; 32],
                };
                write_framed(&mut send, &challenge).await.unwrap();

                // Read auth request
                let auth_req: crate::network::auth::AuthRequest =
                    read_framed(&mut recv).await.unwrap().unwrap();
                assert!(!auth_req.address.is_empty());
                assert_eq!(auth_req.models, vec!["gemma3:4b"]);

                // Send auth response
                let auth_resp = crate::network::auth::AuthResponse {
                    authenticated: true,
                    node_id: Some("test-node-1".into()),
                    error: None,
                };
                write_framed(&mut send, &auth_resp).await.unwrap();

                // Read a NodeMessage
                let msg: NodeMessage = read_framed(&mut recv).await.unwrap().unwrap();
                match msg {
                    NodeMessage::StatusUpdate { version, .. } => {
                        assert_eq!(version, env!("CARGO_PKG_VERSION"));
                    }
                    _ => panic!("expected StatusUpdate"),
                }

                // Signal completion
                let _ = tx.send(());
                server_conn.close(0u32.into(), b"done");
                server_endpoint.close(0u32.into(), b"shutdown");
            });

            // Build client config
            let client_config = build_client_config(true).unwrap();
            let mut client_endpoint =
                Endpoint::client("0.0.0.0:0".parse().unwrap()).unwrap();
            client_endpoint.set_default_client_config(client_config);

            // Connect to server
            let client_conn = client_endpoint
                .connect(server_addr, "localhost")
                .unwrap()
                .await
                .unwrap();

            // Client accepts the bi-stream opened by the server
            let (mut send, mut recv) = client_conn.accept_bi().await.unwrap();

            // Run the auth handshake
            let identity = Identity::from_secret_hex(
                "6472696164726961647269616472696164726961647269616472696164726961",
            )
            .unwrap();

            let node_id = authenticate(
                &mut send,
                &mut recv,
                &identity,
                vec!["gemma3:4b".into()],
                HashMap::from([("gemma3:4b".to_string(), 42.0)]),
                Capacity { free: 1, max: 2 },
            )
            .await
            .unwrap();

            assert_eq!(node_id, "test-node-1");

            // Send a status update
            let status = NodeMessage::StatusUpdate {
                models: vec!["gemma3:4b".into()],
                capacity: Capacity { free: 1, max: 2 },
                version: env!("CARGO_PKG_VERSION").to_string(),
                stats: None,
            };
            write_framed(&mut send, &status).await.unwrap();

            // Wait for server to confirm receipt
            rx.await.expect("server did not signal completion");

            client_conn.close(0u32.into(), b"done");
            client_endpoint.close(0u32.into(), b"shutdown");
        })
        .await
        .expect("test timed out");
    }

    /// Integration test: Full RouterConnection::connect flow with a mock router.
    #[tokio::test]
    async fn test_router_connection_connect() {
        tokio::time::timeout(Duration::from_secs(10), async {
            let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
            let cert_der = CertificateDer::from(cert.cert);
            let key_der =
                rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());

            let mut server_crypto = rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(vec![cert_der.clone()], key_der.into())
                .unwrap();
            server_crypto.alpn_protocols = vec![b"dkn".to_vec()];

            let server_config = quinn::ServerConfig::with_crypto(Arc::new(
                quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto).unwrap(),
            ));

            let server_endpoint =
                Endpoint::server(server_config, "127.0.0.1:0".parse().unwrap()).unwrap();
            let server_addr = server_endpoint.local_addr().unwrap();

            let (tx, rx) = tokio::sync::oneshot::channel::<()>();

            // Mock router: accept connection, open bi-stream, run auth, read one message
            tokio::spawn(async move {
                let incoming = server_endpoint.accept().await.unwrap();
                let server_conn = incoming.await.unwrap();
                let (mut send, mut recv) = server_conn.open_bi().await.unwrap();

                // Challenge-response auth
                write_framed(
                    &mut send,
                    &crate::network::auth::ChallengeMessage {
                        challenge: [0xBB; 32],
                    },
                )
                .await
                .unwrap();

                let _auth_req: crate::network::auth::AuthRequest =
                    read_framed(&mut recv).await.unwrap().unwrap();

                write_framed(
                    &mut send,
                    &crate::network::auth::AuthResponse {
                        authenticated: true,
                        node_id: Some("node-42".into()),
                        error: None,
                    },
                )
                .await
                .unwrap();

                // Send a ping
                write_framed(&mut send, &RouterMessage::Ping).await.unwrap();

                // Read the status update response
                let msg: NodeMessage = read_framed(&mut recv).await.unwrap().unwrap();
                assert!(matches!(msg, NodeMessage::StatusUpdate { .. }));

                let _ = tx.send(());
                server_conn.close(0u32.into(), b"done");
                server_endpoint.close(0u32.into(), b"shutdown");
            });

            // Use RouterConnection::connect
            let url = format!("127.0.0.1:{}", server_addr.port());
            let identity = Identity::from_secret_hex(
                "6472696164726961647269616472696164726961647269616472696164726961",
            )
            .unwrap();

            let mut conn = RouterConnection::connect(
                &url,
                true,
                &identity,
                vec!["gemma3:4b".into()],
                HashMap::from([("gemma3:4b".to_string(), 50.0)]),
                Capacity { free: 2, max: 4 },
            )
            .await
            .unwrap();

            assert_eq!(conn.node_id, "node-42");

            // Receive ping from router
            let msg = conn.recv().await.unwrap().unwrap();
            assert!(matches!(msg, RouterMessage::Ping));

            // Send status update
            conn.send(&NodeMessage::StatusUpdate {
                models: vec!["gemma3:4b".into()],
                capacity: Capacity { free: 2, max: 4 },
                version: env!("CARGO_PKG_VERSION").to_string(),
                stats: None,
            })
            .await
            .unwrap();

            rx.await.expect("server did not signal completion");
            conn.close();
        })
        .await
        .expect("test timed out");
    }

    /// Helper: run the auth handshake as a mock router on an accepted connection.
    async fn mock_router_auth(
        send: &mut quinn::SendStream,
        recv: &mut quinn::RecvStream,
        node_id: &str,
    ) {
        write_framed(
            send,
            &crate::network::auth::ChallengeMessage {
                challenge: [0xCC; 32],
            },
        )
        .await
        .unwrap();

        let _auth_req: crate::network::auth::AuthRequest =
            read_framed(recv).await.unwrap().unwrap();

        write_framed(
            send,
            &crate::network::auth::AuthResponse {
                authenticated: true,
                node_id: Some(node_id.into()),
                error: None,
            },
        )
        .await
        .unwrap();
    }

    /// Helper: build a mock QUIC server endpoint with self-signed cert.
    fn build_mock_server_endpoint() -> Endpoint {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let cert_der = CertificateDer::from(cert.cert);
        let key_der =
            rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());

        let mut server_crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der.clone()], key_der.into())
            .unwrap();
        server_crypto.alpn_protocols = vec![b"dkn".to_vec()];

        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto).unwrap(),
        ));
        let mut transport = TransportConfig::default();
        transport.max_concurrent_bidi_streams(8u32.into());
        server_config.transport_config(Arc::new(transport));

        Endpoint::server(server_config, "127.0.0.1:0".parse().unwrap()).unwrap()
    }

    /// Integration test: full message flow (challenge → auth → ping → status → task assignment → rejection).
    #[tokio::test]
    async fn test_full_message_flow() {
        tokio::time::timeout(Duration::from_secs(10), async {
            let server_endpoint = build_mock_server_endpoint();
            let server_addr = server_endpoint.local_addr().unwrap();

            let (tx, rx) = tokio::sync::oneshot::channel::<()>();

            tokio::spawn(async move {
                let incoming = server_endpoint.accept().await.unwrap();
                let server_conn = incoming.await.unwrap();
                let (mut send, mut recv) = server_conn.open_bi().await.unwrap();

                // Auth
                mock_router_auth(&mut send, &mut recv, "flow-node").await;

                // Send ping
                write_framed(&mut send, &RouterMessage::Ping).await.unwrap();

                // Read status update
                let msg: NodeMessage = read_framed(&mut recv).await.unwrap().unwrap();
                assert!(matches!(msg, NodeMessage::StatusUpdate { .. }));

                // Send a task assignment (node has no real model → rejection expected)
                let task_id = uuid::Uuid::nil();
                write_framed(
                    &mut send,
                    &RouterMessage::TaskAssignment {
                        task_id,
                        model: "nonexistent:1b".into(),
                        messages: vec![dkn_protocol::ChatMessage { role: "user".into(), content: "test".into() }],
                        max_tokens: 10,
                        temperature: 0.7,
                        validation: None,
                    },
                )
                .await
                .unwrap();

                // Read the task rejection
                let reject: NodeMessage = read_framed(&mut recv).await.unwrap().unwrap();
                match reject {
                    NodeMessage::TaskRejected { task_id: tid, reason } => {
                        assert_eq!(tid, task_id);
                        assert!(matches!(reason, dkn_protocol::RejectReason::ModelNotLoaded));
                    }
                    _ => panic!("expected TaskRejected, got {reject:?}"),
                }

                let _ = tx.send(());
                server_conn.close(0u32.into(), b"done");
                server_endpoint.close(0u32.into(), b"shutdown");
            });

            let url = format!("127.0.0.1:{}", server_addr.port());
            let identity = Identity::from_secret_hex(
                "6472696164726961647269616472696164726961647269616472696164726961",
            )
            .unwrap();

            let mut conn = RouterConnection::connect(
                &url,
                true,
                &identity,
                vec!["gemma3:4b".into()],
                HashMap::from([("gemma3:4b".to_string(), 50.0)]),
                Capacity { free: 1, max: 1 },
            )
            .await
            .unwrap();
            assert_eq!(conn.node_id, "flow-node");

            // Receive ping → reply with status
            let msg = conn.recv().await.unwrap().unwrap();
            assert!(matches!(msg, RouterMessage::Ping));

            conn.send(&NodeMessage::StatusUpdate {
                models: vec!["gemma3:4b".into()],
                capacity: Capacity { free: 1, max: 1 },
                version: env!("CARGO_PKG_VERSION").to_string(),
                stats: None,
            })
            .await
            .unwrap();

            // Receive task assignment → we just forward to test; in real code the worker handles it
            let task_msg = conn.recv().await.unwrap().unwrap();
            match task_msg {
                RouterMessage::TaskAssignment { task_id, .. } => {
                    // Reject: model not loaded (only "gemma3:4b" is listed but task asks for "nonexistent:1b")
                    conn.send(&NodeMessage::TaskRejected {
                        task_id,
                        reason: dkn_protocol::RejectReason::ModelNotLoaded,
                    })
                    .await
                    .unwrap();
                }
                _ => panic!("expected TaskAssignment"),
            }

            rx.await.expect("server did not signal completion");
            conn.close();
        })
        .await
        .expect("test timed out");
    }

    /// Integration test: multi-router failover — first server closes immediately, second handles auth.
    #[tokio::test]
    async fn test_multi_router_failover() {
        tokio::time::timeout(Duration::from_secs(10), async {
            // First "bad" server: accepts connection then immediately closes
            let bad_endpoint = build_mock_server_endpoint();
            let bad_addr = bad_endpoint.local_addr().unwrap();

            tokio::spawn(async move {
                if let Some(incoming) = bad_endpoint.accept().await {
                    let conn = incoming.await.unwrap();
                    // Immediately close without opening a stream
                    conn.close(0u32.into(), b"go away");
                    bad_endpoint.close(0u32.into(), b"shutdown");
                }
            });

            // Second "good" server: handles auth normally
            let good_endpoint = build_mock_server_endpoint();
            let good_addr = good_endpoint.local_addr().unwrap();

            let (tx, rx) = tokio::sync::oneshot::channel::<()>();

            tokio::spawn(async move {
                let incoming = good_endpoint.accept().await.unwrap();
                let server_conn = incoming.await.unwrap();
                let (mut send, mut recv) = server_conn.open_bi().await.unwrap();

                mock_router_auth(&mut send, &mut recv, "failover-node").await;

                let _ = tx.send(());
                // Keep connection alive until test finishes
                tokio::time::sleep(Duration::from_secs(2)).await;
                server_conn.close(0u32.into(), b"done");
                good_endpoint.close(0u32.into(), b"shutdown");
            });

            let identity = Identity::from_secret_hex(
                "6472696164726961647269616472696164726961647269616472696164726961",
            )
            .unwrap();

            // Try bad server first, then good server
            let urls = vec![
                format!("127.0.0.1:{}", bad_addr.port()),
                format!("127.0.0.1:{}", good_addr.port()),
            ];

            let mut connected = None;
            for url in &urls {
                match RouterConnection::connect(
                    url,
                    true,
                    &identity,
                    vec!["gemma3:4b".into()],
                    HashMap::from([("gemma3:4b".to_string(), 50.0)]),
                    Capacity { free: 1, max: 1 },
                )
                .await
                {
                    Ok(conn) => {
                        connected = Some(conn);
                        break;
                    }
                    Err(_) => continue,
                }
            }

            let conn = connected.expect("should have connected to second server");
            assert_eq!(conn.node_id, "failover-node");

            rx.await.expect("server did not signal completion");
            conn.close();
        })
        .await
        .expect("test timed out");
    }

    /// Integration test: stats field is present in StatusUpdate.
    #[tokio::test]
    async fn test_status_update_with_stats() {
        tokio::time::timeout(Duration::from_secs(10), async {
            let server_endpoint = build_mock_server_endpoint();
            let server_addr = server_endpoint.local_addr().unwrap();

            let (tx, rx) = tokio::sync::oneshot::channel::<()>();

            tokio::spawn(async move {
                let incoming = server_endpoint.accept().await.unwrap();
                let server_conn = incoming.await.unwrap();
                let (mut send, mut recv) = server_conn.open_bi().await.unwrap();

                mock_router_auth(&mut send, &mut recv, "stats-node").await;

                // Send ping
                write_framed(&mut send, &RouterMessage::Ping).await.unwrap();

                // Read status update — verify stats field is present
                let msg: NodeMessage = read_framed(&mut recv).await.unwrap().unwrap();
                match msg {
                    NodeMessage::StatusUpdate { stats, version, .. } => {
                        assert_eq!(version, env!("CARGO_PKG_VERSION"));
                        let s = stats.expect("stats should be present");
                        assert_eq!(s.tasks_completed, 42);
                        assert_eq!(s.total_tokens_generated, 1000);
                    }
                    _ => panic!("expected StatusUpdate"),
                }

                let _ = tx.send(());
                server_conn.close(0u32.into(), b"done");
                server_endpoint.close(0u32.into(), b"shutdown");
            });

            let url = format!("127.0.0.1:{}", server_addr.port());
            let identity = Identity::from_secret_hex(
                "6472696164726961647269616472696164726961647269616472696164726961",
            )
            .unwrap();

            let mut conn = RouterConnection::connect(
                &url,
                true,
                &identity,
                vec!["gemma3:4b".into()],
                HashMap::from([("gemma3:4b".to_string(), 50.0)]),
                Capacity { free: 1, max: 1 },
            )
            .await
            .unwrap();

            // Receive ping
            let msg = conn.recv().await.unwrap().unwrap();
            assert!(matches!(msg, RouterMessage::Ping));

            // Send status update with stats
            conn.send(&NodeMessage::StatusUpdate {
                models: vec!["gemma3:4b".into()],
                capacity: Capacity { free: 1, max: 1 },
                version: env!("CARGO_PKG_VERSION").to_string(),
                stats: Some(dkn_protocol::NodeStatsSnapshot {
                    tasks_completed: 42,
                    tasks_failed: 3,
                    tasks_rejected: 1,
                    total_tokens_generated: 1000,
                    uptime_secs: 600,
                }),
            })
            .await
            .unwrap();

            rx.await.expect("server did not signal completion");
            conn.close();
        })
        .await
        .expect("test timed out");
    }
}
