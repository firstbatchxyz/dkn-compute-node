# Dria Node v2 — Tester Guide

Thanks for testing! This guide walks you through building and running a Dria compute node from source.

## 1. Install Prerequisites

You need **Rust** and **cmake**. Pick your OS:

### macOS

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install cmake
brew install cmake
```

### Linux (Ubuntu/Debian)

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install build tools
sudo apt-get update && sudo apt-get install -y cmake build-essential
```

### Windows

Open **PowerShell as Administrator** (right-click Start → "Terminal (Admin)") and run these commands one by one:

```powershell
# Install Rust
winget install Rustlang.Rustup

# Install C++ build tools (needed to compile the inference engine)
winget install Microsoft.VisualStudio.2022.BuildTools --force --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools;includeRecommended"

# Install CMake
winget install -e --id Kitware.CMake

# Install LLVM/Clang (needed by bindgen for llama.cpp bindings)
winget install -e --id LLVM.LLVM
```

**Important:** After all three finish, **close PowerShell and open a new one** so the tools are available. To verify everything installed correctly:

```powershell
rustc --version
cmake --version
```

Both should print a version number. If either says "not recognized", restart your PC and try again.

## 2. Build

```bash
git clone https://github.com/firstbatchxyz/dkn-compute-node.git
cd dkn-compute-node
git checkout v2
cargo build --release
```

This takes a few minutes (it compiles the inference engine from source).

**Apple Silicon (M1/M2/M3/M4)?** Build with Metal GPU support instead:

```bash
cargo build --release --features metal
```

**NVIDIA GPU?** Install the [CUDA Toolkit](https://developer.nvidia.com/cuda-downloads) first, then:

```bash
cargo build --release --features cuda
```

## 3. Run Setup

The setup wizard helps you pick and download a model:

```bash
./target/release/dria-node setup
```

**Windows (PowerShell):** Use backslashes and `.exe`:

```powershell
.\target\release\dria-node.exe setup
```

It will:
- Detect your RAM and show models that fit
- Let you pick a model and quantization
- Download it (once, cached for future runs)
- Run a test inference to confirm everything works

If you're unsure which model to pick, start with **lfm2.5:1.2b** — it's the smallest (~0.8 GB) and works on any machine.

## 4. Your Wallet Key

You'll need your Ethereum wallet private key. The node uses it to sign messages and prove identity on the network.

This is the 64-character hex string (with or without `0x` prefix). You can export it from MetaMask: Account Details → Show Private Key.

## 5. Start the Node

```bash
./target/release/dria-node start \
  --wallet YOUR_KEY_HERE \
  --model lfm2.5:1.2b
```

**Windows (PowerShell):**

```powershell
.\target\release\dria-node.exe start --wallet YOUR_KEY_HERE --model lfm2.5:1.2b
```

Replace `YOUR_KEY_HERE` with the key from step 4, and `lfm2.5:1.2b` with whatever model you chose in setup.

**If you have a GPU** and built with `--features metal` or `--features cuda`:

```bash
./target/release/dria-node start \
  --wallet YOUR_KEY_HERE \
  --model lfm2.5:1.2b \
  --gpu-layers -1
```

### What to expect

```
INFO node identity                         address=0x...
INFO benchmark complete                    tps=25.3 model=lfm2.5:1.2b
INFO connected to router                   node_id=... router=quic.dria.co:4001
INFO node ready                            models=["lfm2.5:1.2b"] online=true
```

That's it — the node is running and accepting tasks. Leave it open. Press **Ctrl+C** to stop.

## 6. Skip the Flags Next Time

Instead of typing flags every time, set environment variables:

```bash
# Add these to your shell profile (~/.bashrc, ~/.zshrc, etc.)
export DRIA_WALLET=your_key_here
export DRIA_MODELS=lfm2.5:1.2b
export DRIA_GPU_LAYERS=-1
```

Then just run:

```bash
./target/release/dria-node start
```

## Models

| Model | Type | Download | Min RAM |
|---|---|---|---|
| qwen3.5:0.8b | Text, Vision | ~0.5 GB | ~1 GB |
| lfm2.5:1.2b | Text | ~0.8 GB | ~1 GB |
| lfm2.5-audio:1.5b | Text, Audio | ~1.0 GB | ~1.5 GB |
| lfm2.5-vl:1.6b | Text, Vision | ~1.2 GB | ~1.5 GB |
| qwen3.5:2b | Text, Vision | ~1.2 GB | ~2 GB |
| nanbeige:3b | Text | ~2.0 GB | ~2.5 GB |
| locooperator:4b | Text | ~2.5 GB | ~3 GB |
| qwen3.5:9b | Text, Vision | ~6.0 GB | ~7 GB |
| lfm2:24b-a2b | Text | ~14 GB | ~16 GB |
| qwen3.5:27b | Text, Vision | ~16 GB | ~18 GB |
| qwen3.5:35b-a3b | Text, Vision | ~20 GB | ~22 GB |
| nemotron:30b-a3b | Text | ~24.5 GB | ~27 GB |

Pick one model that fits your RAM. Smaller models are faster to download and easier to test with.

## All Options

| Flag | Env Var | Default | What it does |
|---|---|---|---|
| `--wallet` | `DRIA_WALLET` | (required) | Your node identity key |
| `--model` | `DRIA_MODELS` | (required) | Model(s) to serve |
| `--router-url` | `DRIA_ROUTER_URL` | `quic.dria.co:4001` | Router to connect to |
| `--gpu-layers` | `DRIA_GPU_LAYERS` | `0` (CPU) | GPU layers (-1 = all) |
| `--max-concurrent` | `DRIA_MAX_CONCURRENT` | `1` | Parallel inference tasks |
| `--data-dir` | `DRIA_DATA_DIR` | `~/.dria` | Where models are cached |
| `--quant` | `DRIA_QUANT` | Q4_K_M | Override quantization |
| `--insecure` | `DRIA_INSECURE` | `false` | Skip TLS verification |
| `--skip-update` | `DRIA_SKIP_UPDATE` | `false` | Skip auto-update check |

## Troubleshooting

**Windows: "dria-node is not recognized"**
On Windows you must use `.\target\release\dria-node.exe` (backslashes, `.exe` extension). PowerShell does not find executables without the `.exe` suffix.

**"cmake not found" or build errors about C compiler**
Make sure cmake is installed (step 1). On macOS: `brew install cmake`. On Linux: `sudo apt install cmake build-essential`. On Windows: `winget install -e --id Kitware.CMake` then reopen PowerShell.

**Windows: "dria-node.exe not found in target\release"**
The build probably failed. Scroll up in your terminal and look for red error messages. The most common cause is missing C++ build tools — run `winget install Microsoft.VisualStudio.2022.BuildTools --force --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools;includeRecommended"`, reopen PowerShell, and rebuild with `cargo build --release`.

**Windows: "Unable to find libclang" or "couldn't find clang.dll"**
Install LLVM: `winget install -e --id LLVM.LLVM`, reopen PowerShell, and rebuild. If it still can't find it, set the path manually: `$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"` then rebuild.

**Build fails**
Try a clean build: `cargo clean && cargo build --release`. Make sure you're on the `v2` branch: `git checkout v2`.

**"unknown model"**
Model names are exact. Use the names from the table above (e.g. `lfm2.5:1.2b`, not `lfm-2.5`).

**"all routers unavailable" or "offline mode"**
The node can't reach the router. Check your internet connection. If you're behind a strict firewall, **UDP port 4001 outbound** must be allowed.

**Slow inference**
If you have a GPU, make sure you built with `--features metal` (Mac) or `--features cuda` (NVIDIA) and are passing `--gpu-layers -1`.

**Model download stalls or fails**
Models come from HuggingFace. Try again — it might be a temporary network issue. You can also set `HF_ENDPOINT` if HuggingFace is blocked in your region.

**Want more detail in the logs?**

```bash
RUST_LOG=debug ./target/release/dria-node start ...
```

## Reporting Issues

If something goes wrong, please share:
1. Your OS and hardware (CPU, RAM, GPU)
2. The command you ran
3. The full error output
