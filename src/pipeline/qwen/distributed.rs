//! The 2-process sharded pipeline: [`ShardedChain`](super::shard::ShardedChain)'s in-process
//! hand-off, but the hop to the downstream shards goes over the QUIC
//! [`transport`](crate::pipeline::transport).
//!
//! Roles:
//!   - **Head** (node-0, path owner): runs its local shards (first embeds tokens), ships the
//!     residual stream to the tail via [`QuicDownstream`], gets back the per-position argmax.
//!     [`RemoteChain`] implements [`TargetChain`], so the spec-decode driver drives it unchanged.
//!   - **Tail**: [`serve_tail`] runs the last shard and replies with the argmax over its logits.
//!
//! Threading: the Luminal runtime is `!Send`, and the spec-decode driver is sync, while QUIC is
//! async. So each side owns its shard on one OS thread and `block_on`s the async hops there (the
//! head per hop; the tail in a serve loop). The Luminal future never migrates threads.
#![allow(dead_code)] // head/tail are consumed by the integration test + (next) the worker.

use std::io;
use std::net::SocketAddr;

use quinn::Endpoint;

use super::shard::Shard;
use super::target::{argmax, QwenRuntime};
use crate::pipeline::engine::{TargetChain, TokenId};
use crate::pipeline::specpipe::PipelinedChain;
use crate::pipeline::transport::{
    accept, dial, dial_endpoint, HiddenStateFrame, PeerMessage, PeerConnection, VerifyResult,
};

const EOS_TOKEN: TokenId = 151645; // <|im_end|>
const STOP_TOKEN: TokenId = 151643; // <|endoftext|>

/// The head's synchronous link to the downstream (remote) shards: one persistent QUIC bi-stream,
/// driven by an owned current-thread runtime so each hop is a plain blocking call from the sync
/// spec-decode driver.
pub struct QuicDownstream {
    rt: tokio::runtime::Runtime,
    // Held to keep the client side driving I/O; dropping it would stall the connection.
    _endpoint: Endpoint,
    peer: PeerConnection,
    request_id: u64,
}

impl QuicDownstream {
    /// Dial the tail at `addr` and establish the bi-stream.
    pub fn connect(addr: SocketAddr) -> io::Result<Self> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let (endpoint, peer) = rt.block_on(async move {
            let ep = dial_endpoint()?;
            let peer = dial(&ep, addr).await?;
            io::Result::Ok((ep, peer))
        })?;
        Ok(Self { rt, _endpoint: endpoint, peer, request_id: 0 })
    }

    /// Ship the post-head-shards residual stream (`s` positions from `start_pos`, dim `hidden_dim`)
    /// to the tail; block on the tail's per-position argmax.
    pub fn forward(&mut self, start_pos: u32, s: usize, hidden_dim: usize, hidden: Vec<f32>) -> io::Result<Vec<TokenId>> {
        self.request_id += 1;
        let frame = HiddenStateFrame::from_f32(
            self.request_id,
            0,
            start_pos as u64,
            start_pos,
            0,
            s as u32,
            hidden_dim as u32,
            &hidden,
        );
        let peer = &mut self.peer;
        self.rt.block_on(async move {
            peer.send(&PeerMessage::Hidden(frame)).await?;
            match peer.recv().await? {
                Some(PeerMessage::Result(r)) => Ok(r.argmax),
                Some(other) => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("tail returned {other:?}, expected Result"),
                )),
                None => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "tail closed mid-request")),
            }
        })
    }

    /// End the session cleanly: finish the send half (the tail sees clean EOF and returns), then
    /// drive the runtime until the tail closes back. Must run the runtime so the FIN is flushed.
    pub fn shutdown(&mut self) {
        let peer = &mut self.peer;
        self.rt.block_on(async move {
            peer.finish();
            peer.wait_closed().await;
        });
    }
}

/// The head node: local head shards + a remote tail. Implements [`TargetChain`] so the
/// spec-decode driver drives the distributed model exactly like a single-process one.
pub struct RemoteChain<R: QwenRuntime> {
    head: Vec<Shard<R>>, // local shards; head[0] embeds tokens
    down: QuicDownstream,
    hidden_dim: usize,
}

impl<R: QwenRuntime + 'static> RemoteChain<R> {
    pub fn new(head: Vec<Shard<R>>, down: QuicDownstream) -> Self {
        assert!(!head.is_empty(), "the head needs at least one local shard");
        let hidden_dim = head[0].hidden_dim();
        Self { head, down, hidden_dim }
    }

    /// Run `tokens` through the local head shards → residual stream, ship to the tail → the
    /// tail's per-position argmax.
    fn run(&mut self, start_pos: u32, tokens: &[TokenId]) -> Vec<TokenId> {
        let s = tokens.len();
        let mut hidden = self.head[0].forward_tokens(start_pos, tokens);
        for shard in self.head.iter_mut().skip(1) {
            hidden = shard.forward_hidden(start_pos, s, hidden);
        }
        self.down
            .forward(start_pos, s, self.hidden_dim, hidden)
            .expect("downstream hop failed")
    }

    pub fn reset(&mut self) {
        for shard in &mut self.head {
            shard.reset();
        }
        // The tail resets its own KV on a fresh request_id boundary in this minimal version; for
        // the test we drive a single generation per connection.
    }
}

impl<R: QwenRuntime + 'static> TargetChain for RemoteChain<R> {
    fn prefill(&mut self, prompt: &[TokenId]) -> TokenId {
        *self.run(0, prompt).last().expect("non-empty prompt")
    }

    fn verify(&mut self, start_pos: u32, cur: TokenId, drafts: &[TokenId]) -> Vec<TokenId> {
        let mut chunk = Vec::with_capacity(drafts.len() + 1);
        chunk.push(cur);
        chunk.extend_from_slice(drafts);
        self.run(start_pos, &chunk)
    }

    fn truncate(&mut self, _keep_to_pos: u32) {
        // No-op: every shard (head + tail) writes KV at the same absolute positions.
    }

    fn is_eos(&self, token: TokenId) -> bool {
        token == EOS_TOKEN || token == STOP_TOKEN
    }
}

/// Run the tail: own `shard` (the last shard) on this thread, accept one inbound peer, and serve
/// hidden→argmax over the QUIC link until the head closes. The Luminal runtime stays on this
/// thread (the serve future is `block_on`'d, never spawned); the quinn endpoint's I/O driver runs
/// on the runtime's worker threads so it keeps ticking during the (blocking) shard compute.
pub fn serve_tail<R: QwenRuntime + 'static>(
    mut shard: Shard<R>,
    endpoint: Endpoint,
    rt: &tokio::runtime::Runtime,
) -> io::Result<()> {
    assert!(shard.is_last(), "the tail must be the last shard (it owns the lm_head)");
    let vocab = shard.vocab_size();
    rt.block_on(async move {
        let mut peer = accept(&endpoint).await?;
        while let Some(msg) = peer.recv().await? {
            match msg {
                PeerMessage::Hidden(frame) => {
                    let s = frame.n_tokens as usize;
                    let hidden = frame.to_f32();
                    let logits = shard.forward_hidden(frame.start_pos, s, hidden); // sync GPU
                    let toks = (0..s)
                        .map(|j| argmax(&logits[j * vocab..(j + 1) * vocab]))
                        .collect();
                    peer.send(&PeerMessage::Result(VerifyResult {
                        request_id: frame.request_id,
                        step: frame.step,
                        argmax: toks,
                    }))
                    .await?;
                }
                PeerMessage::Truncate(_) => {} // no-op: position-indexed KV
                PeerMessage::Result(_) => break,
            }
        }
        // Clean EOF from the head (its finish()): return; dropping `peer` closes back to the head.
        io::Result::Ok(())
    })
}

/// Run a middle shard: dial the downstream node, accept the upstream peer, and for each hidden
/// frame run the local shard, forward the new hidden downstream, and relay the result back
/// upstream (return-through-chain). This is the 2-node hop composed recursively — a chain of any
/// length is head → serve_middle → … → serve_middle → serve_tail.
pub fn serve_middle<R: QwenRuntime + 'static>(
    mut shard: Shard<R>,
    upstream: Endpoint,
    downstream_addr: SocketAddr,
    rt: &tokio::runtime::Runtime,
) -> io::Result<()> {
    assert!(!shard.is_last(), "a middle shard must not be the last shard");
    let hidden_dim = shard.hidden_dim();
    rt.block_on(async move {
        // Establish the downstream link first (it's already listening), then wait for the head.
        let down_ep = dial_endpoint()?;
        let mut down = dial(&down_ep, downstream_addr).await?;
        let mut up = accept(&upstream).await?;
        let mut req = 0u64;
        while let Some(msg) = up.recv().await? {
            let frame = match msg {
                PeerMessage::Hidden(f) => f,
                PeerMessage::Truncate(_) => continue,
                PeerMessage::Result(_) => break,
            };
            let s = frame.n_tokens as usize;
            let out = shard.forward_hidden(frame.start_pos, s, frame.to_f32()); // sync GPU
            req += 1;
            let down_frame = HiddenStateFrame::from_f32(
                req,
                frame.epoch,
                frame.step,
                frame.start_pos,
                frame.layer_boundary + 1,
                s as u32,
                hidden_dim as u32,
                &out,
            );
            down.send(&PeerMessage::Hidden(down_frame)).await?;
            let result = match down.recv().await? {
                Some(PeerMessage::Result(r)) => r,
                Some(other) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("downstream returned {other:?}, expected Result"),
                    ))
                }
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "downstream closed mid-request",
                    ))
                }
            };
            // Relay upstream, preserving the upstream's request_id/step.
            up.send(&PeerMessage::Result(VerifyResult {
                request_id: frame.request_id,
                step: frame.step,
                argmax: result.argmax,
            }))
            .await?;
        }
        // Clean EOF from upstream → propagate the FIN downstream, then return (drop closes upstream).
        down.finish();
        down.wait_closed().await;
        io::Result::Ok(())
    })
}

/// The head as a depth-N **pipelined** chain (the throughput path): `submit` fires a hidden frame
/// downstream without waiting; `poll` reads the tail's results FIFO. Over one bi-stream the tail
/// (`serve_tail`, unchanged) processes frames in submission order, so submit/poll stay FIFO. The
/// win: the head's shard compute for chunk k+1 overlaps the tail's compute for chunk k. `truncate`
/// is a no-op — after a divergence the driver re-submits from the committed boundary and every
/// shard's absolute-position KV write overwrites the stale tail (same property as the sync path).
pub struct PipelinedQuicChain<R: QwenRuntime> {
    head: Vec<Shard<R>>,
    hidden_dim: usize,
    rt: tokio::runtime::Runtime,
    _endpoint: Endpoint,
    peer: PeerConnection,
    step: u64,
}

impl<R: QwenRuntime + 'static> PipelinedQuicChain<R> {
    pub fn connect(head: Vec<Shard<R>>, tail_addr: SocketAddr) -> io::Result<Self> {
        assert!(!head.is_empty(), "the head needs at least one local shard");
        let hidden_dim = head[0].hidden_dim();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        let (endpoint, peer) = rt.block_on(async move {
            let ep = dial_endpoint()?;
            let peer = dial(&ep, tail_addr).await?;
            io::Result::Ok((ep, peer))
        })?;
        Ok(Self { head, hidden_dim, rt, _endpoint: endpoint, peer, step: 0 })
    }

    fn run_head(&mut self, start_pos: u32, tokens: &[TokenId]) -> Vec<f32> {
        let s = tokens.len();
        let mut hidden = self.head[0].forward_tokens(start_pos, tokens);
        for shard in self.head.iter_mut().skip(1) {
            hidden = shard.forward_hidden(start_pos, s, hidden);
        }
        hidden
    }

    fn send_hidden(&mut self, start_pos: u32, s: usize, hidden: Vec<f32>) {
        self.step += 1;
        let frame = HiddenStateFrame::from_f32(
            self.step,
            0,
            self.step,
            start_pos,
            0,
            s as u32,
            self.hidden_dim as u32,
            &hidden,
        );
        let peer = &mut self.peer;
        self.rt
            .block_on(async move { peer.send(&PeerMessage::Hidden(frame)).await })
            .expect("pipelined submit send");
    }

    fn recv_result(&mut self) -> Vec<TokenId> {
        let peer = &mut self.peer;
        self.rt
            .block_on(async move {
                match peer.recv().await? {
                    Some(PeerMessage::Result(r)) => Ok(r.argmax),
                    Some(other) => Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("tail returned {other:?}, expected Result"),
                    )),
                    None => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "tail closed mid-request")),
                }
            })
            .expect("pipelined poll recv")
    }

    pub fn shutdown(&mut self) {
        let peer = &mut self.peer;
        self.rt.block_on(async move {
            peer.finish();
            peer.wait_closed().await;
        });
    }
}

impl<R: QwenRuntime + 'static> PipelinedChain for PipelinedQuicChain<R> {
    fn prefill(&mut self, prompt: &[TokenId]) -> TokenId {
        let hidden = self.run_head(0, prompt);
        self.send_hidden(0, prompt.len(), hidden);
        *self.recv_result().last().expect("non-empty prompt")
    }

    fn submit(&mut self, start_pos: u32, cur: TokenId, drafts: &[TokenId]) {
        let mut chunk = Vec::with_capacity(drafts.len() + 1);
        chunk.push(cur);
        chunk.extend_from_slice(drafts);
        let hidden = self.run_head(start_pos, &chunk); // sync GPU; overlaps the tail's prior chunk
        self.send_hidden(start_pos, chunk.len(), hidden);
    }

    fn poll(&mut self) -> Vec<TokenId> {
        self.recv_result()
    }

    fn truncate(&mut self, _keep_to_pos: u32) {
        // No-op: re-submit from the committed boundary overwrites every shard's stale KV tail.
    }

    fn is_eos(&self, token: TokenId) -> bool {
        token == EOS_TOKEN || token == STOP_TOKEN
    }
}

#[cfg(all(test, feature = "metal", target_vendor = "apple"))]
mod tests {
    use super::*;
    use luminal::prelude::Runtime; // MetalRuntime::initialize is a Runtime-trait method
    use crate::pipeline::qwen::hf::prepare_hf_model;
    use crate::pipeline::qwen::model::QwenConfig;
    use crate::pipeline::qwen::target::QwenTarget;
    use crate::pipeline::transport::listen_endpoint;

    fn greedy<T: TargetChain>(t: &mut T, prompt: &[TokenId], max_new: usize) -> Vec<TokenId> {
        let mut cur = t.prefill(prompt);
        let mut out = Vec::new();
        let mut base = prompt.len() as u32;
        while !t.is_eos(cur) && out.len() < max_new {
            out.push(cur);
            cur = t.verify(base, cur, &[])[0];
            base += 1;
        }
        out
    }

    // THE 2-PROCESS PIPELINE: head (shard 0) and tail (shard 1) in separate threads, the hidden
    // state crossing a REAL QUIC link, must reproduce the single-process forward bit-for-bit.
    // cargo test -p dria-node --features metal qwen_pipeline_over_quic -- --ignored --nocapture
    #[test]
    #[ignore]
    fn qwen_pipeline_over_quic() {
        let cfg = QwenConfig::QWEN3_0_6B;
        let dir = prepare_hf_model("Qwen/Qwen3-0.6B").expect("prepare 0.6B");
        let wp = dir.join("model_combined_bf16_v1.safetensors").to_str().unwrap().to_string();
        let k = cfg.layers / 2;

        // Tail thread: bind the endpoint inside its own runtime, report the addr early (before the
        // slow shard load, so the head's loads overlap), then load shard 1 and serve.
        let (addr_tx, addr_rx) = std::sync::mpsc::channel();
        let wp_tail = wp.clone();
        let tail = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap();
            let endpoint = {
                let _g = rt.enter();
                listen_endpoint("127.0.0.1:0".parse().unwrap()).unwrap()
            };
            addr_tx.send(endpoint.local_addr().unwrap()).unwrap();
            let shard1 = Shard::load(
                luminal_metal::MetalRuntime::initialize(()),
                cfg,
                (k, cfg.layers),
                false,
                true,
                &wp_tail,
                512,
                200,
            );
            serve_tail(shard1, endpoint, &rt).expect("tail serve");
        });

        // Single-process baseline.
        let mut full =
            QwenTarget::load(luminal_metal::MetalRuntime::initialize(()), cfg, &wp, 512, 200);

        // Head: shard 0, dial the tail, build the distributed chain.
        let shard0 = Shard::load(
            luminal_metal::MetalRuntime::initialize(()),
            cfg,
            (0, k),
            true,
            false,
            &wp,
            512,
            200,
        );
        let addr = addr_rx.recv().expect("tail addr");
        let down = QuicDownstream::connect(addr).expect("dial tail");
        let mut chain = RemoteChain::new(vec![shard0], down);

        let prompt: Vec<TokenId> = vec![40, 1079, 264, 4108, 4108];
        let max_new = 24;

        full.reset();
        let base = greedy(&mut full, &prompt, max_new);
        chain.reset();
        let distributed = greedy(&mut chain, &prompt, max_new);

        chain.down.shutdown();
        tail.join().expect("tail thread");

        println!("\nfull (1 proc)        = {base:?}");
        println!("pipeline (2 nodes/QUIC) = {distributed:?}");
        match (0..base.len().min(distributed.len())).find(|&i| base[i] != distributed[i]) {
            None if base.len() == distributed.len() => {
                println!("LOSSLESS PIPELINE: head + tail over QUIC == single-process forward")
            }
            Some(i) => println!("diverge @{i}: full={} pipe={}", base[i], distributed[i]),
            None => println!("length differ: full={} pipe={}", base.len(), distributed.len()),
        }
        assert_eq!(base, distributed, "the 2-process pipeline must be lossless");
    }

    // N-NODE CHAIN: head + middle + tail (0.6B split in thirds) across three threads, the hidden
    // state crossing TWO QUIC links, must equal the single-process forward.
    // cargo test -p dria-node --features metal qwen_pipeline_3node_over_quic -- --ignored --nocapture
    #[test]
    #[ignore]
    fn qwen_pipeline_3node_over_quic() {
        use std::sync::mpsc;
        let cfg = QwenConfig::QWEN3_0_6B;
        let dir = prepare_hf_model("Qwen/Qwen3-0.6B").expect("prepare 0.6B");
        let wp = dir.join("model_combined_bf16_v1.safetensors").to_str().unwrap().to_string();
        let l = cfg.layers;
        let a = l / 3; // head [0,a), middle [a,b), tail [b,l)
        let b = 2 * l / 3;

        let mk_rt = || {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap()
        };

        // Tail (last shard).
        let (tail_tx, tail_rx) = mpsc::channel();
        let wp_t = wp.clone();
        let tail = std::thread::spawn(move || {
            let rt = mk_rt();
            let ep = {
                let _g = rt.enter();
                listen_endpoint("127.0.0.1:0".parse().unwrap()).unwrap()
            };
            tail_tx.send(ep.local_addr().unwrap()).unwrap();
            let shard = Shard::load(
                luminal_metal::MetalRuntime::initialize(()),
                cfg,
                (b, l),
                false,
                true,
                &wp_t,
                512,
                200,
            );
            serve_tail(shard, ep, &rt).expect("tail serve");
        });
        let tail_addr = tail_rx.recv().expect("tail addr");

        // Middle shard.
        let (mid_tx, mid_rx) = mpsc::channel();
        let wp_m = wp.clone();
        let middle = std::thread::spawn(move || {
            let rt = mk_rt();
            let ep = {
                let _g = rt.enter();
                listen_endpoint("127.0.0.1:0".parse().unwrap()).unwrap()
            };
            mid_tx.send(ep.local_addr().unwrap()).unwrap();
            let shard = Shard::load(
                luminal_metal::MetalRuntime::initialize(()),
                cfg,
                (a, b),
                false,
                false,
                &wp_m,
                512,
                200,
            );
            serve_middle(shard, ep, tail_addr, &rt).expect("middle serve");
        });
        let mid_addr = mid_rx.recv().expect("middle addr");

        // Baseline + head.
        let mut full =
            QwenTarget::load(luminal_metal::MetalRuntime::initialize(()), cfg, &wp, 512, 200);
        let shard0 = Shard::load(
            luminal_metal::MetalRuntime::initialize(()),
            cfg,
            (0, a),
            true,
            false,
            &wp,
            512,
            200,
        );
        let down = QuicDownstream::connect(mid_addr).expect("dial middle");
        let mut chain = RemoteChain::new(vec![shard0], down);

        let prompt: Vec<TokenId> = vec![40, 1079, 264, 4108, 4108];
        let max_new = 24;
        full.reset();
        let base = greedy(&mut full, &prompt, max_new);
        chain.reset();
        let dist = greedy(&mut chain, &prompt, max_new);

        chain.down.shutdown();
        middle.join().expect("middle thread");
        tail.join().expect("tail thread");

        println!("\nfull (1 proc)           = {base:?}");
        println!("pipeline (3 nodes/QUIC) = {dist:?}");
        match (0..base.len().min(dist.len())).find(|&i| base[i] != dist[i]) {
            None if base.len() == dist.len() => {
                println!("LOSSLESS 3-NODE PIPELINE: head + middle + tail over QUIC == single-process")
            }
            Some(i) => println!("diverge @{i}: full={} pipe={}", base[i], dist[i]),
            None => println!("length differ: full={} pipe={}", base.len(), dist.len()),
        }
        assert_eq!(base, dist, "the 3-node pipeline must be lossless");
    }

    /// Prompt-lookup n-gram draft — exercises accept AND reject (the divergence/discard path).
    struct NgramDraft;
    impl crate::pipeline::engine::DraftEngine for NgramDraft {
        fn draft(&mut self, context: &[TokenId], gamma: usize) -> Vec<TokenId> {
            let n = context.len();
            if n >= 2 {
                let (a, b) = (context[n - 2], context[n - 1]);
                for i in (0..n - 2).rev() {
                    if context[i] == a && context[i + 1] == b {
                        let start = i + 2;
                        let mut d: Vec<TokenId> =
                            (0..gamma).filter_map(|k| context.get(start + k).copied()).collect();
                        if !d.is_empty() {
                            while d.len() < gamma {
                                d.push(*d.last().unwrap());
                            }
                            return d;
                        }
                    }
                }
            }
            vec![*context.last().unwrap_or(&0); gamma]
        }
    }

    // PIPELINED over QUIC: the depth-N pipelined driver over the real 2-node chain must stay
    // lossless vs greedy — exercises submit/poll FIFO + the no-op truncate across divergences.
    // cargo test -p dria-node --features metal qwen_pipelined_over_quic -- --ignored --nocapture
    #[test]
    #[ignore]
    fn qwen_pipelined_over_quic() {
        use crate::pipeline::specdecode::SpecConfig;
        use crate::pipeline::specpipe::run_spec_decode_pipelined;
        let cfg = QwenConfig::QWEN3_0_6B;
        let dir = prepare_hf_model("Qwen/Qwen3-0.6B").expect("prepare 0.6B");
        let wp = dir.join("model_combined_bf16_v1.safetensors").to_str().unwrap().to_string();
        let k = cfg.layers / 2;

        let (tail_tx, tail_rx) = std::sync::mpsc::channel();
        let wp_t = wp.clone();
        let tail = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap();
            let ep = {
                let _g = rt.enter();
                listen_endpoint("127.0.0.1:0".parse().unwrap()).unwrap()
            };
            tail_tx.send(ep.local_addr().unwrap()).unwrap();
            let shard = Shard::load(
                luminal_metal::MetalRuntime::initialize(()),
                cfg,
                (k, cfg.layers),
                false,
                true,
                &wp_t,
                512,
                200,
            );
            serve_tail(shard, ep, &rt).expect("tail serve");
        });
        let tail_addr = tail_rx.recv().expect("tail addr");

        let mut full =
            QwenTarget::load(luminal_metal::MetalRuntime::initialize(()), cfg, &wp, 512, 200);
        let shard0 = Shard::load(
            luminal_metal::MetalRuntime::initialize(()),
            cfg,
            (0, k),
            true,
            false,
            &wp,
            512,
            200,
        );
        let mut chain = PipelinedQuicChain::connect(vec![shard0], tail_addr).expect("connect tail");

        let prompt: Vec<TokenId> = vec![40, 1079, 264, 4108, 4108];
        let spec_cfg = SpecConfig { gamma: 4, max_new: 24 };

        full.reset();
        let base = greedy(&mut full, &prompt, 24);

        let mut draft = NgramDraft;
        let (out, stats) = run_spec_decode_pipelined(&mut draft, &mut chain, &prompt, &spec_cfg, 4);
        chain.shutdown();
        tail.join().expect("tail thread");

        println!("\nfull greedy               = {base:?}");
        println!("pipelined (depth 4, QUIC) = {out:?}");
        println!("toks/traversal = {:.2}", stats.toks_per_traversal());
        match (0..base.len().min(out.len())).find(|&i| base[i] != out[i]) {
            None if base.len() == out.len() => {
                println!("LOSSLESS PIPELINED over QUIC == greedy (depth-N driver + no-op truncate hold)")
            }
            Some(i) => println!("diverge @{i}: full={} pipe={}", base[i], out[i]),
            None => println!("length differ: full={} pipe={}", base.len(), out.len()),
        }
        assert_eq!(base, out, "the pipelined QUIC chain must be lossless vs greedy");
    }

    /// Perfect draft: returns the target's own greedy continuation, so the pipeline never flushes.
    struct PerfectDraft {
        truth: Vec<TokenId>,
        prompt_len: usize,
    }
    impl crate::pipeline::engine::DraftEngine for PerfectDraft {
        fn draft(&mut self, context: &[TokenId], gamma: usize) -> Vec<TokenId> {
            let l = context.len() - self.prompt_len;
            (0..gamma).map(|j| *self.truth.get(l + j).unwrap_or(&0)).collect()
        }
    }

    // THROUGHPUT: depth-1 (no pipelining; one chunk in flight) vs depth-4 (pipelined) over the
    // 2-node QUIC chain, with a perfect draft so the pipeline stays full. On loopback (RTT≈0) the
    // win is compute overlap (head's shard for chunk k+1 runs while the tail does chunk k); over
    // real WAN it additionally hides the chain RTT. Same chain/connection — each run's prefill at
    // pos 0 overwrites the KV, so no reset needed (position-indexed).
    // cargo test -p dria-node --features metal qwen_pipeline_throughput -- --ignored --nocapture
    #[test]
    #[ignore]
    fn qwen_pipeline_throughput() {
        use crate::pipeline::specdecode::SpecConfig;
        use crate::pipeline::specpipe::run_spec_decode_pipelined;
        use std::time::Instant;
        let cfg = QwenConfig::QWEN3_0_6B;
        let dir = prepare_hf_model("Qwen/Qwen3-0.6B").expect("prepare 0.6B");
        let wp = dir.join("model_combined_bf16_v1.safetensors").to_str().unwrap().to_string();
        let k = cfg.layers / 2;

        let (tail_tx, tail_rx) = std::sync::mpsc::channel();
        let wp_t = wp.clone();
        let tail = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap();
            let ep = {
                let _g = rt.enter();
                listen_endpoint("127.0.0.1:0".parse().unwrap()).unwrap()
            };
            tail_tx.send(ep.local_addr().unwrap()).unwrap();
            let shard = Shard::load(
                luminal_metal::MetalRuntime::initialize(()),
                cfg,
                (k, cfg.layers),
                false,
                true,
                &wp_t,
                768,
                200,
            );
            serve_tail(shard, ep, &rt).expect("tail serve");
        });
        let tail_addr = tail_rx.recv().expect("tail addr");

        // Truth = the target's greedy continuation (drives the perfect draft).
        let mut full =
            QwenTarget::load(luminal_metal::MetalRuntime::initialize(()), cfg, &wp, 768, 200);
        let prompt: Vec<TokenId> = vec![40, 1079, 264, 4108, 4108];
        let n = 500;
        full.reset();
        let truth = greedy(&mut full, &prompt, n);

        let shard0 = Shard::load(
            luminal_metal::MetalRuntime::initialize(()),
            cfg,
            (0, k),
            true,
            false,
            &wp,
            768,
            200,
        );
        let mut chain = PipelinedQuicChain::connect(vec![shard0], tail_addr).expect("connect tail");
        let spec_cfg = SpecConfig { gamma: 6, max_new: truth.len() };
        let mut draft = PerfectDraft { truth: truth.clone(), prompt_len: prompt.len() };

        // Warm-up (first run pays one-time per-shape costs).
        let _ = run_spec_decode_pipelined(&mut draft, &mut chain, &prompt, &spec_cfg, 1);

        let t0 = Instant::now();
        let (out1, st1) = run_spec_decode_pipelined(&mut draft, &mut chain, &prompt, &spec_cfg, 1);
        let dt1 = t0.elapsed();

        let t0 = Instant::now();
        let (out4, st4) = run_spec_decode_pipelined(&mut draft, &mut chain, &prompt, &spec_cfg, 4);
        let dt4 = t0.elapsed();

        chain.shutdown();
        tail.join().expect("tail thread");

        let tps1 = out1.len() as f64 / dt1.as_secs_f64();
        let tps4 = out4.len() as f64 / dt4.as_secs_f64();
        println!("\ndepth-1 (no pipelining): {} tok in {:.3}s = {:.1} tok/s ({} traversals)", out1.len(), dt1.as_secs_f64(), tps1, st1.traversals);
        println!("depth-4 (pipelined):     {} tok in {:.3}s = {:.1} tok/s ({} traversals)", out4.len(), dt4.as_secs_f64(), tps4, st4.traversals);
        println!("PIPELINING SPEEDUP: {:.2}x (loopback ⇒ compute-overlap only; WAN adds RTT-hiding)", tps4 / tps1);
        assert_eq!(out1, truth, "depth-1 must be lossless");
        assert_eq!(out4, truth, "depth-4 must be lossless");
    }
}
