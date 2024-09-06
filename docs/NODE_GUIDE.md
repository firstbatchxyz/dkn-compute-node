## Running the Compute Node

Running a Dria Compute Node is pretty straightforward! You can either follow the guide here for all platforms, or follow a much-more user-friendly guide at <https://dria.co/guide> for MacOS in particular.

## Requirements

### Software

Depending the AI models of your choice, you may have to install software:

- **OpenAI models**: you don't have to do anything!
- **Ollama models**: you have to install Ollama

```sh
# prints Ollama version
ollama -v
```

### Hardware

**To learn about hardware specifications such as required CPU and RAM, please refer to [node specifications](./NODE_SPECS.md).**

In general, if you are using Ollama you will need the memory to run large models locally, which depend on the model's size that you are willing to. If you are in a memory-constrained environment, you can opt to use OpenAI models instead.

> [!NOTE]
>
> The compute node is a lightweight process, but you may see increased memory & CPU usage during the initial testing phases, due to various protocol-level operations with the growing network size.

## Setup

To be able to run a node, we need to make a few simple preparations. Follow the steps below one by one.

### 1. Download [Launcher](https://github.com/firstbatchxyz/dkn-compute-launcher)

We have a [cross-platform node launcher](https://github.com/firstbatchxyz/dkn-compute-launcher) to easily set up the environment and running the compute node. We will install that first.

Download the appropriate ZIP file for your system using the commands below or from [browser](https://github.com/firstbatchxyz/dkn-compute-launcher/releases/tag/v0.0.1). Make sure to replace the URL with the correct version for your operating system and architecture.

#### macOS:

1. Check your architecture:

   ```sh
   uname -m
   ```

   - If the output is `arm64`, download the `arm64` version.
   - If it's `x86_64`, download the `amd64` version.

2. Download the ZIP file:

   ```sh
   # for arm64, use arm64
   curl -L -o dkn-compute-node.zip https://github.com/firstbatchxyz/dkn-compute-launcher/releases/latest/download/dkn-compute-launcher-macOS-arm64.zip
   ```

   ```sh
   # for x86_64, use amd64
   curl -L -o dkn-compute-node.zip https://github.com/firstbatchxyz/dkn-compute-launcher/releases/latest/download/dkn-compute-launcher-macOS-amd64.zip
   ```

3. Unzip the downloaded file:
   ```sh
   unzip dkn-compute-node.zip
   cd dkn-compute-node
   ```

> [!TIP]
>
> Some devices need you to bypass macOS's security warning. If you see "macOS cannot verify that this app is free from malware," when running the node use the following command:
>
> ```sh
> xattr -d com.apple.quarantine dkn-compute-launcher
> ```

#### Linux:

1. Check your architecture:

   ```sh
   uname -m
   ```

   - If it's `aarch64`, download the `arm64` version.
   - If the output is `x86_64`, download the `amd64` version.

2. Download the ZIP file:

   ```sh
   # for aarch64, use arm64
   curl -L -o dkn-compute-node.zip https://github.com/firstbatchxyz/dkn-compute-launcher/releases/latest/download/dkn-compute-launcher-linux-arm64.zip
   ```

   ```sh
   # for x86_64, use amd64
   curl -L -o dkn-compute-node.zip https://github.com/firstbatchxyz/dkn-compute-launcher/releases/latest/download/dkn-compute-launcher-linux-amd64.zip
   ```

3. Unzip the downloaded file:
   ```sh
   unzip dkn-compute-node.zip
   cd dkn-compute-node
   ```

#### Windows:

1. Check your architecture:

   - Open System Information:
     - Press <kbd>⊞ Win + R</kbd> to open the Run dialog.
     - Type `msinfo32` and press <kbd>Enter</kbd>.
   - Look for the line labeled "Processor" or "CPU":
     - If it includes "x64" or refers to Intel or AMD, it is likely x86 (amd64).
     - If it mentions ARM, then it's an ARM processor.

2. Download the ZIP file using a web browser or in PowerShell:

   ```sh
   # for x64, use amd64
   Invoke-WebRequest -Uri "https://github.com/firstbatchxyz/dkn-compute-launcher/releases/latest/download/dkn-compute-launcher-windows-amd64.zip" -OutFile "dkn-compute-node.zip"
   ```

   ```sh
   # for ARM, use arm64
   Invoke-WebRequest -Uri "https://github.com/firstbatchxyz/dkn-compute-launcher/releases/latest/download/dkn-compute-launcher-windows-arm64.zip" -OutFile "dkn-compute-node.zip"
   ```

3. Unzip the downloaded file using File Explorer or in PowerShell:
   ```sh
   Expand-Archive -Path "dkn-compute-node.zip" -DestinationPath "dkn-compute-node"
   cd dkn-compute-node
   ```

### 2. Prepare Environment Variables

With our launcher, setting up the environment variables happen on the fly! The CLI application will ask you to enter the required environment variables if you don't have them.

This way, you won't have to manually do the copying and creating environment variables yourself, and instead let the CLI do it for you. You can move directly on to the [Usage](#usage) section.

> If you would like to do this part manually, you can continue reading this section.

#### Create `.env` File

Dria Compute Node makes use of several environment variables. Let's create an `.env` file from the given example first.

```sh
cp .env.example .ev
```

We will fill out the missing parts witin `.env` file in a moment.

> [!NOTE]
>
> `DKN_ADMIN_PUBLIC_KEY` is used to verify that the tasks are given by certain nodes, so that your node does not work for tasks given to the network by untrusted people. You don't need to change this, simply copy and paste it to your `.env`.

> [!TIP]
>
> While adding anything to your `.env`, you can do it without leaving the terminal. For example, suppose you want to set `VALUE` to some `KEY`, you can do it as:
>
> ```sh
> echo "KEY=VALUE" >> .env
> ```
>
> If you would like to view the `.env` without leaving the terminal, you can do:
>
> ```sh
> cat .env
> ```

### 3. Prepare Ethereum Wallet

Dria makes use of the same Ethereum wallet, that is the recipient of your hard-earned rewards! Place your private key at `DKN_WALLET_SECRET_KEY` in `.env` without the `0x` prefix. It should look something like:

```sh
DKN_WALLET_SECRET_KEY=ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

> [!CAUTION]
>
> Always make sure your private key is within the .gitignore'd `.env` file, nowhere else! To be even safer, you can use a throw-away wallet, you can always transfer your claimed rewards to a main wallet afterwards.

### 4. Setup LLM Provider

For the final step, we need to make sure we can serve LLM requests.

#### For OpenAI

If you will be using OpenAI to serve its models, you need to have an API key in the environment. Simply set the key within your `.env`:

```sh
OPENAI_API_KEY=<YOUR_KEY>
```

#### For Ollama

First you have to install [Ollama](#requirements), if you haven't already! The compute node is set to download any missing model automatically at the start by default. This is enabled via the `OLLAMA_AUTO_PULL=true` in `.env`.

If you would like to disable this feature, set `OLLAMA_AUTO_PULL=false` and then continue reading this section, otherwise you can skip to [optional services](#optional-services).

First, you must **first pull a small embedding model that is used internally**.

```sh
ollama pull hellord/mxbai-embed-large-v1:f16
```

For the models that you choose (see list of models just below [here](#1-choose-models)) you can download them with same command. Note that if your model size is large, pulling them may take a while. For example:

```sh
# example
ollama pull llama3.1:latest
```

#### Optional Services

Based on presence of API keys, [Ollama Workflows](https://github.com/andthattoo/ollama-workflows/) may use more superior services instead of free alternatives, e.g. [Serper](https://serper.dev/) instead of [DuckDuckGo](https://duckduckgo.com/) or [Jina](https://jina.ai/) without rate-limit instead of with rate-limit. Add these within your `.env` as:

```sh
SERPER_API_KEY=<key-here>
JINA_API_KEY=<key-here>
```

## Usage

**With all setup steps above completed, we are ready to start a node!** Either double-click the downloaded launcher `dkn-compute-launcher` app (`dkn-compute-launcher.exe` on Windows), or run it from the terminal from your file explorer, or use it from terminal (or `cmd/powershell` in Windows).

See the available commands with:

```sh
# macos or linux
./dkn-compute-launcher --help
```

```sh
# windows
.\dkn-compute-launcher.exe --help
```

Then simply run the cli app, it will ask you to enter required inputs:

```sh
# macos or linux
./dkn-compute-launcher
```

```sh
# windows
.\dkn-compute-launcher.exe
```

You will see logs of the compute node on the same terminal!

You can stop the node as usual by pressing <kbd>Control + C</kbd>, or kill it from the terminal.

### Choosing Models

You will be asked to provide your choice of models within the CLI. You can also pass them from the command line using `-m` flags:

```sh
# macos or linux
./dkn-compute-launcher -m=llama3.1:latest -m=gpt-3.5-turbo
```

```sh
# windows
.\dkn-compute-launcher.exe -m=llama3.1:latest -m=gpt-3.5-turbo
```

[Available models](https://github.com/andthattoo/ollama-workflows/blob/main/src/program/models.rs) are given below:

#### Ollama Models

- `finalend/hermes-3-llama-3.1:8b-q8_0`
- `phi3:14b-medium-4k-instruct-q4_1`
- `phi3:14b-medium-128k-instruct-q4_1`
- `phi3.5:3.8b`
- `phi3.5:3.8b-mini-instruct-fp16`
- `llama3.1:latest`
- `llama3.1:8b-instruct-q8_0`
- `gemma2:9b-instruct-q8_0`

#### OpenAI Models

- `gpt-3.5-turbo`
- `gpt-4-turbo`
- `gpt-4o`
- `gpt-4o-mini`

### Additional Static Nodes

You can add additional relay nodes & bootstrap nodes from environment, using the `DKN_RELAY_NODES` and `DKN_BOOTSTRAP_NODES` variables respectively. Simply write the `Multiaddr` string of the static nodes as comma-separated values, and the compute node will pick them up at the start.

```sh
# dummy example
DKN_BOOTSTRAP_NODES=/ip4/44.206.245.139/tcp/4001/p2p/16Uiu2HAm4q3LZU2TeeejKK4fff6KZdddq8Kcccyae4bbbF7uqaaa
```
