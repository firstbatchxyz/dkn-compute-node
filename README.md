<p align="center">
  <img src="https://raw.githubusercontent.com/firstbatchxyz/.github/refs/heads/master/branding/dria-logo-square.svg" alt="logo" width="168">
</p>

<p align="center">
  <h1 align="center">
    Dria Compute Node
  </h1>
  <p align="center">
    <i>Run AI inference on the Dria network. Earn rewards by serving models from your machine.</i>
  </p>
</p>

<p align="center">
    <a href="https://opensource.org/license/apache-2-0" target="_blank">
        <img alt="License: Apache-2.0" src="https://img.shields.io/badge/license-Apache%202.0-7CB9E8.svg">
    </a>
    <a href="./.github/workflows/test.yml" target="_blank">
        <img alt="Workflow: Tests" src="https://github.com/firstbatchxyz/dkn-compute-node/actions/workflows/tests.yml/badge.svg?branch=master">
    </a>
    <a href="https://github.com/firstbatchxyz/dkn-compute-node/releases" target="_blank">
        <img alt="Downloads" src="https://img.shields.io/github/downloads/firstbatchxyz/dkn-compute-node/total?logo=github&logoColor=%23F2FFEE&color=%2332C754">
    </a>
    <a href="https://dria.co/discord" target="_blank">
        <img alt="Discord" src="https://dcbadge.vercel.app/api/server/dria?style=flat">
    </a>
</p>

## Quick Start

### Install

Choose one installation method:

**Homebrew (macOS / Linux):**

```sh
brew install firstbatchxyz/dkn/dria-node
dria-node --version
```

Homebrew will add the tap automatically.

**Shell script (macOS / Linux):**

```sh
curl -fsSL https://raw.githubusercontent.com/firstbatchxyz/dkn-compute-node/master/install.sh | sh
dria-node --version
```

**AMD ROCm (Linux x86_64):**

```sh
curl -fsSL https://raw.githubusercontent.com/firstbatchxyz/dkn-compute-node/master/install-rocm.sh | bash
dria-node --version
```

Requires ROCm 6.x to already be installed on your machine.

**PowerShell (Windows):**

```powershell
irm https://raw.githubusercontent.com/firstbatchxyz/dkn-compute-node/master/install.ps1 | iex
dria-node --version
```

**From GitHub Releases:**

Download the latest file for your platform from [Releases](https://github.com/firstbatchxyz/dkn-compute-node/releases), then run `dria-node --version` to verify it.

### Setup

Run the interactive setup:

```sh
dria-node setup
```

This will:

1. Detect your system RAM and list models that fit
2. Let you pick a model from the available options
3. Download the GGUF model file from HuggingFace
4. Run a test inference to verify everything works
5. Print a benchmark (tokens per second)

Use `--gpu-layers -1` to offload all layers to GPU (Metal on macOS, CUDA on NVIDIA builds, ROCm on AMD Linux builds):

```sh
dria-node setup --gpu-layers -1
```

### Start

Once setup is complete, start the node:

```sh
dria-node start --wallet <YOUR_SECRET_KEY> --model <MODEL_NAME>
```

The node will connect to the Dria network, register your models, and start serving inference requests. You can increase throughput with `--max-concurrent`:

```sh
dria-node start --wallet <KEY> --model lfm2.5:1.2b --max-concurrent 4
```

## Available Models

| Model | Type | Quant | ~Size |
|-------|------|-------|-------|
| `lfm2.5:1.2b` | Text | Q4_K_M | 0.8 GB |
| `lfm2.5-audio:1.5b` | Audio | Q4_0 | 1.0 GB |
| `lfm2.5-vl:1.6b` | Vision | Q4_0 | 1.2 GB |
| `nanbeige:3b` | Text | Q4_K_M | 2.0 GB |
| `locooperator:4b` | Text | Q4_K_M | 2.5 GB |
| `qwen3.5:9b` | Vision | Q4_K_M | 6.0 GB |
| `lfm2:24b-a2b` | Text | Q4_K_M | 14 GB |
| `qwen3.5:27b` | Vision | Q4_K_M | 16 GB |
| `qwen3.5:35b-a3b` | Vision | Q4_K_M | 20 GB |

Serve multiple models by comma-separating them: `--model "qwen3.5:9b,lfm2.5:1.2b"`

Override quantization with `--quant Q8_0` (applies to all models).

## CLI Reference

```
dria-node <COMMAND>

Commands:
  setup    Interactive setup: pick a model, download it, and run a test
  start    Start the compute node

setup options:
  --data-dir <PATH>        Data directory [env: DRIA_DATA_DIR]
  --gpu-layers <N>         GPU layers to offload (0 = CPU only) [default: 0]

start options:
  --wallet <KEY>           Wallet secret key, hex-encoded [env: DRIA_WALLET]
  --model <MODELS>         Model(s) to serve, comma-separated [env: DRIA_MODELS]
  --router-url <URL>       Router URL [default: quic.dria.co:4001] [env: DRIA_ROUTER_URL]
  --gpu-layers <N>         GPU layers to offload (-1 = all, 0 = CPU) [default: 0]
  --max-concurrent <N>     Max concurrent inference requests [default: 1]
  --data-dir <PATH>        Data directory [env: DRIA_DATA_DIR]
  --quant <QUANT>          Override GGUF quantization [env: DRIA_QUANT]
  --insecure               Skip TLS verification [env: DRIA_INSECURE]
```

All flags can also be set via environment variables.

## Building from Source

```sh
git clone https://github.com/firstbatchxyz/dkn-compute-node.git
cd dkn-compute-node
cargo build --release
```

**Feature flags:**

- `--features metal` — Apple Metal GPU acceleration (macOS)
- `--features cuda` — NVIDIA CUDA GPU acceleration
- `--features rocm` — AMD ROCm GPU acceleration (Linux x86_64)

### Testing

```sh
cargo test
```

### Linting

```sh
cargo clippy
cargo fmt --check
```

## License

This project is licensed under the [Apache License 2.0](https://opensource.org/license/Apache-2.0).
