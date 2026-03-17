# DKN Network Testing Guide

How to test the full router + compute-node stack locally (single machine) and over the internet (two laptops).

## Prerequisites

- Rust toolchain (`rustup`, `cargo`)
- `openssl` CLI (for generating TLS certs)
- A HuggingFace account (models download automatically)
- ~1 GB free disk for the smallest model (`lfm2.5:1.2b`)

Build both binaries first:

```bash
# Router
cd dkn-router && cargo build --release

# Compute node (CPU)
cd dkn-compute-node && cargo build --release

# Compute node (Metal / Apple Silicon)
cd dkn-compute-node && cargo build --release --features metal

# Compute node (CUDA)
cd dkn-compute-node && cargo build --release --features cuda
```

## Generate a wallet key

Any 32-byte hex string works as a test wallet:

```bash
openssl rand -hex 32
# example output: a1b2c3d4...64 hex chars total
```

Save it — you'll pass it to the node via `--wallet`.

---

## Scenario 1: Everything on localhost

### 1. Generate self-signed TLS certs

```bash
mkdir -p /tmp/dkn-certs

openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout /tmp/dkn-certs/key.pem -out /tmp/dkn-certs/cert.pem \
  -days 365 -nodes -subj "/CN=localhost" \
  -addext "subjectAltName=DNS:localhost,IP:127.0.0.1"
```

### 2. Start the router

```bash
./dkn-router/target/release/dkn-router \
  --listen-quic 127.0.0.1:4001 \
  --listen-http 127.0.0.1:8080 \
  --cert /tmp/dkn-certs/cert.pem \
  --key  /tmp/dkn-certs/key.pem
```

You should see:

```
INFO starting DKN router quic=127.0.0.1:4001 http=127.0.0.1:8080
INFO router ready ...
```

### 3. Start the compute node

In a second terminal:

```bash
./dkn-compute-node/target/release/dria-node start \
  --wallet <YOUR_HEX_KEY> \
  --model  lfm2.5:1.2b \
  --router-url https://127.0.0.1:4001 \
  --insecure \
  --gpu-layers -1
```

- `--insecure` skips TLS verification (required for self-signed certs).
- `--gpu-layers -1` offloads all layers to GPU. Use `0` for CPU-only.
- First run downloads the model from HuggingFace (~730 MB).

You should see:

```
INFO node identity address=0x...
INFO model found in cache ...
INFO benchmark complete tps=... model=lfm2.5:1.2b
INFO connected to router node_id=... router=https://127.0.0.1:4001
INFO node ready ...
```

### 4. Send a request

```bash
curl -s http://127.0.0.1:8080/v1/generate \
  -H "Content-Type: application/json" \
  -d '{
    "model": "lfm2.5:1.2b",
    "messages": [{"role": "user", "content": "What is 2+2?"}],
    "max_tokens": 128,
    "temperature": 0.7
  }' | python3 -m json.tool
```

Expected response:

```json
{
    "text": "2+2 equals 4...",
    "model": "lfm2.5:1.2b",
    "stats": {
        "tokens_generated": 12,
        "prompt_tokens": 8,
        "generation_time_ms": 450,
        "tokens_per_second": 26.7
    }
}
```

### 5. Check other endpoints

```bash
# Health check
curl -s http://127.0.0.1:8080/v1/health | python3 -m json.tool

# List models served by connected nodes
curl -s http://127.0.0.1:8080/v1/models | python3 -m json.tool

# Batch request
curl -s http://127.0.0.1:8080/v1/batch \
  -H "Content-Type: application/json" \
  -d '{
    "tasks": [
      {"model": "lfm2.5:1.2b", "messages": [{"role": "user", "content": "Say hi"}]},
      {"model": "lfm2.5:1.2b", "messages": [{"role": "user", "content": "Say bye"}]}
    ],
    "timeout_secs": 30
  }' | python3 -m json.tool
```

### 6. Run multiple nodes (optional)

Start a second node with a different model and wallet on the same machine:

```bash
./dkn-compute-node/target/release/dria-node start \
  --wallet $(openssl rand -hex 32) \
  --model  nanbeige:3b \
  --router-url https://127.0.0.1:4001 \
  --insecure \
  --gpu-layers 0
```

Now `/v1/models` will show both `lfm2.5:1.2b` and `nanbeige:3b`.

---

## Scenario 2: Two laptops over the internet

**Laptop A** = router, **Laptop B** = compute node.

### 1. Find Laptop A's public IP

If Laptop A is behind NAT (home router), you need to either:

- **Port-forward** UDP 4001 and TCP 8080 on the home router to Laptop A's LAN IP.
- Use a cloud VM (DigitalOcean, AWS, etc.) as Laptop A instead.

Get the public IP:

```bash
curl -s ifconfig.me
# e.g. 203.0.113.42
```

### 2. Generate TLS certs on Laptop A

Generate certs with the public IP as a SAN:

```bash
export ROUTER_IP=203.0.113.42  # replace with your public IP

mkdir -p /tmp/dkn-certs

openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout /tmp/dkn-certs/key.pem -out /tmp/dkn-certs/cert.pem \
  -days 365 -nodes -subj "/CN=$ROUTER_IP" \
  -addext "subjectAltName=IP:$ROUTER_IP"
```

If you have a domain name, use `DNS:yourdomain.com` instead of `IP:...`.

### 3. Start the router on Laptop A

```bash
./dkn-router/target/release/dkn-router \
  --listen-quic 0.0.0.0:4001 \
  --listen-http 0.0.0.0:8080 \
  --cert /tmp/dkn-certs/cert.pem \
  --key  /tmp/dkn-certs/key.pem
```

Note `0.0.0.0` to listen on all interfaces.

### 4. Verify connectivity from Laptop B

```bash
# Check HTTP is reachable
curl -s http://203.0.113.42:8080/v1/health

# Check QUIC port is open (UDP)
nc -z -u 203.0.113.42 4001 && echo "open" || echo "blocked"
```

If the health check returns `{"status":"ok",...}`, HTTP is working. If QUIC is blocked, check firewall/NAT rules for **UDP** port 4001.

### 5. Start the compute node on Laptop B

```bash
./dkn-compute-node/target/release/dria-node start \
  --wallet <YOUR_HEX_KEY> \
  --model  lfm2.5:1.2b \
  --router-url https://203.0.113.42:4001 \
  --insecure \
  --gpu-layers -1
```

`--insecure` is needed because the cert is self-signed. Once the node connects:

```
INFO connected to router node_id=... router=https://203.0.113.42:4001
```

### 6. Send requests from either laptop

From Laptop A (or any machine that can reach the router):

```bash
curl -s http://203.0.113.42:8080/v1/generate \
  -H "Content-Type: application/json" \
  -d '{
    "model": "lfm2.5:1.2b",
    "messages": [{"role": "user", "content": "Hello from the internet!"}],
    "max_tokens": 64
  }' | python3 -m json.tool
```

The HTTP request goes to the router, which forwards it via QUIC to the node on Laptop B, which runs inference and sends the result back.

---

## Scenario 3: LAN testing (two laptops, same network)

Same as Scenario 2 but simpler — no NAT/port-forwarding needed.

### 1. Find Laptop A's LAN IP

```bash
# macOS
ipconfig getifaddr en0

# Linux
hostname -I | awk '{print $1}'
```

Example: `192.168.1.100`

### 2. Generate certs and start router on Laptop A

```bash
export ROUTER_IP=192.168.1.100

mkdir -p /tmp/dkn-certs

openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout /tmp/dkn-certs/key.pem -out /tmp/dkn-certs/cert.pem \
  -days 365 -nodes -subj "/CN=$ROUTER_IP" \
  -addext "subjectAltName=IP:$ROUTER_IP"

./dkn-router/target/release/dkn-router \
  --listen-quic 0.0.0.0:4001 \
  --listen-http 0.0.0.0:8080 \
  --cert /tmp/dkn-certs/cert.pem \
  --key  /tmp/dkn-certs/key.pem
```

### 3. Start node on Laptop B

```bash
./dkn-compute-node/target/release/dria-node start \
  --wallet $(openssl rand -hex 32) \
  --model  lfm2.5:1.2b \
  --router-url https://192.168.1.100:4001 \
  --insecure \
  --gpu-layers -1
```

### 4. Send requests from Laptop A

```bash
curl -s http://192.168.1.100:8080/v1/generate \
  -H "Content-Type: application/json" \
  -d '{
    "model": "lfm2.5:1.2b",
    "messages": [{"role": "user", "content": "Hello from LAN!"}],
    "max_tokens": 64
  }' | python3 -m json.tool
```

---

## Available models

| Short name | Size | Type | Notes |
|---|---|---|---|
| `lfm2.5:1.2b` | 731 MB | text | Fastest, good for testing |
| `nanbeige:3b` | 2.4 GB | text | |
| `locooperator:4b` | 2.5 GB | text | |
| `lfm2.5-vl:1.6b` | 696 MB | vision | Rejects text-only requests are fine, rejects audio |
| `lfm2.5-audio:1.5b` | 696 MB | audio | Rejects image content |
| `lfm2:24b-a2b` | 14.4 GB | text | MoE |
| `qwen3.5:27b` | 16.7 GB | text | |
| `qwen3.5:35b-a3b` | 19.9 GB | text | MoE |

## Environment variables

All CLI flags can be set via env vars instead:

| Env var | Flag | Default |
|---|---|---|
| `DRIA_WALLET` | `--wallet` | (required) |
| `DRIA_MODELS` | `--model` | (required) |
| `DRIA_ROUTER_URL` | `--router-url` | `https://router.dria.co` |
| `DRIA_GPU_LAYERS` | `--gpu-layers` | `0` |
| `DRIA_MAX_CONCURRENT` | `--max-concurrent` | `1` |
| `DRIA_DATA_DIR` | `--data-dir` | `~/.dria` |
| `DRIA_INSECURE` | `--insecure` | `false` |
| `DRIA_ROUTER_QUIC_ADDR` | `--listen-quic` | `0.0.0.0:4001` |
| `DRIA_ROUTER_HTTP_ADDR` | `--listen-http` | `0.0.0.0:8080` |
| `DRIA_ROUTER_CERT` | `--cert` | (required) |
| `DRIA_ROUTER_KEY` | `--key` | (required) |

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Node logs `all routers unavailable` | Can't reach router QUIC port | Check firewall allows **UDP** 4001, verify IP/port |
| Node logs `TLS error` | Cert doesn't match router hostname/IP | Regenerate cert with correct SAN, or use `--insecure` |
| `curl` to `/v1/generate` returns 503 | No nodes connected | Check node logs, ensure it says `connected to router` |
| `curl` to `/v1/generate` returns 504 | Node timeout during inference | Increase `timeout_secs` in request, or use a smaller model |
| Node logs `SHA-256 mismatch` | Corrupted download | Delete `~/.dria/models/` and restart to re-download |
| `QUIC connect failed: no initial cipher suite` | TLS/QUIC version mismatch | Ensure both router and node are built from the same branch |
| Batch request partial failures | One model not loaded | Check `/v1/models` to see what's available |

## Verbose logging

```bash
RUST_LOG=debug ./target/release/dkn-router ...
RUST_LOG=debug ./target/release/dria-node start ...
```
