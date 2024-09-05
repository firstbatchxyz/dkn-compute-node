## Running the Compute Node

Running a Dria Compute Node is pretty straightforward! You can either follow the guide here for all platforms, or follow a much-more user-friendly guide at <https://dria.co/guide> for MacOS in particular.

## Requirements

### Software

You only **Docker** to run the node! You can check if you have it by printing its version:

```sh
docker -v
```

> [!CAUTION]
>
> In **Windows** machines, Docker Desktop is requried to be running with **WSL2**. You can check the Docker Desktop Windows installation guide from [here](https://docs.docker.com/desktop/install/windows-install/)

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
   curl -L -o dkn-compute-node.zip https://github.com/firstbatchxyz/dkn-compute-launcher/releases/download/v0.0.1/dkn-compute-launcher-macOS-arm64.zip
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

   - If the output is `x86_64`, download the `amd64` version.
   - If it's `aarch64`, download the `arm64` version.

2. Download the ZIP file:

   ```sh
   curl -L -o dkn-compute-node.zip https://github.com/firstbatchxyz/dkn-compute-launcher/releases/download/v0.0.1/dkn-compute-launcher-linux-amd64.zip
   ```

3. Unzip the downloaded file:
   ```sh
   unzip dkn-compute-node.zip
   cd dkn-compute-node
   ```

#### Windows:

1. Check your architecture:

   - Open System Information:
     - Press `Win + R` to open the Run dialog.
     - Type `msinfo32` and press Enter.
   - Look for the line labeled "Processor" or "CPU":
     - If it includes "x64" or refers to Intel or AMD, it is likely x86 (amd64).
     - If it mentions ARM, then it's an ARM processor.

2. Download the ZIP file using a web browser or in PowerShell:

   ```cmd
   Invoke-WebRequest -Uri "https://github.com/firstbatchxyz/dkn-compute-launcher/releases/download/v0.0.1/dkn-compute-launcher-windows-amd64.zip" -OutFile "dkn-compute-node.zip"
   ```

3. Unzip the downloaded file using File Explorer or in PowerShell:
   ```cmd
   Expand-Archive -Path "dkn-compute-node.zip" -DestinationPath "dkn-compute-node"
   cd dkn-compute-node
   ```

### 2. Prepare Environment Variables

With our launcher, setting up the environment variables happen on the fly by just running the `dkn-compute-launcher` CLI application directly, it'll ask you to enter the required environment variables if you don't have them! This way, you won't have to manually do the copying and creating environment variables yourself, and instead let the CLI do it for you.

If you prefer this method, you can move directly on to the [Usage](#usage) section. If you would like to do this part manually, you can continue reading this section.

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

First you have to install Ollama, if you haven't already! See their [download page](https://ollama.com/download) and follow their instructions there. The models that we want to use have to be pulled to Ollama before we can use them.

> [!TIP]
>
> The compute node is set to download any missing model automatically at the start by default. This is enabled via the `OLLAMA_AUTO_PULL=true` in `.env`. If you would like to disable this feature, set `OLLAMA_AUTO_PULL=false` and then continue reading this section, otherwise you can skip to [optional services](#optional-services).

First, you must **first pull a small embedding model that is used internally**.

```sh
ollama pull hellord/mxbai-embed-large-v1:f16
```

For the models that you choose (see list of models just below [here](#1-choose-models)) you can download them with same command. Note that if your model size is large, pulling them may take a while. For example:

```sh
# example
ollama pull llama3.1:latest
```

> [!TIP]

#### Optional Services

Based on presence of API keys, [Ollama Workflows](https://github.com/andthattoo/ollama-workflows/) may use more superior services instead of free alternatives, e.g. [Serper](https://serper.dev/) instead of [DuckDuckGo](https://duckduckgo.com/) or [Jina](https://jina.ai/) without rate-limit instead of with rate-limit. Add these within your `.env` as:

```sh
SERPER_API_KEY=<key-here>
JINA_API_KEY=<key-here>
```

## Usage

With all setup steps above completed, we are ready to start a node!

### 1. Choose Model(s)

Based on the resources of your machine, you must decide which models that you will be running locally. For example, you can use OpenAI with their models, not running anything locally at all; or you can use Ollama with several models loaded to disk, and only one loaded to memory during its respective task. Available models (see [here](https://github.com/andthattoo/ollama-workflows/blob/main/src/program/atomics.rs#L269) for latest) are:

#### Ollama Models

- `adrienbrault/nous-hermes2theta-llama3-8b:q8_0`
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

> [!TIP]
>
> If you are using Ollama, make sure you have pulled the required models, as specified in the [section above](#4-setup-ollama-for-ollama-users)!

### 2. Start Docker

Our node will be running within a Docker container, so we should make sure that Docker is running before the next step. You can launch Docker via its [desktop application](https://www.docker.com/products/docker-desktop/), or a command such as:

```sh
sudo systemctl start docker
```

> [!NOTE]
>
> You don't need to do this step if Docker is already running in the background.

### 3. Run Node

It's time to run our compute node. We have a launcher cli app that makes this much easier: you can either run it by double-clicking the `dkn-compute-launcher` app (`dkn-compute-launcher.exe` on Windows) from your file explorer, or use it from terminal (or cmd/powershell in Windows).

See the available commands with:

```sh
# macos or linux
./dkn-compute-launcher --help

# windows
.\dkn-compute-launcher.exe --help
```

Then simply run the cli app, it will ask you to enter required inputs:

```sh
# macos or linux
./dkn-compute-launcher

# windows
.\dkn-compute-launcher.exe
```

Or you can directly pass the running models using `-m` flags

```sh
# macos or linux
./dkn-compute-launcher -m=llama3.1:latest -m=gpt-3.5-turbo

# windows
.\dkn-compute-launcher.exe -m=llama3.1:latest -m=gpt-3.5-turbo
```

Launcher app will run the containers in the background. You can check their logs either via the terminal or from [Docker Desktop](https://www.docker.com/products/docker-desktop/).

#### Running in Debug Mode

To print DEBUG-level logs for the compute node, you can add `--dev` argument to the launcher app. For example:

```sh
./dkn-compute-launcher -m=gpt-4o-mini --dev
```

Running in debug mode will also allow you to see behind the scenes of Ollama Workflows, i.e. you can see the reasoning of the LLM as it executes the task.

> Similarly, you can run in trace mode with `--trace` to see trace logs, which cover low-level logs from the p2p client.

### 4. Looking at Logs

To see your logs, you can go to [Docker Desktop](https://www.docker.com/products/docker-desktop/) and see the running containers and find `dkn-compute-node`. There, open the containers within the compose (click on `>` to the left) and click on any of the container to see its logs.

Alternatively, you can use `docker compose logs` such as below:

```sh
docker compose logs -f compute  # compute node logs
docker compose logs -f ollama   # ollama logs
```

The `-f` option is so that you can track the logs from terminal. If you prefer to simply check the latest logs, you can use a command such as:

```sh
# logs from last 1 hour
docker compose logs --since=1h compute

# logs from last 30 minutes
docker compose logs --since=30m compute
```

### 5. Stopping the Node

When you start your node with `dkn-compute-launcher`, it will wait for you in the same terminal to do CTRL+C before stopping. Once you do that, the containers will be stopped and removed. You can also kill the containers manually, doing CTRL+C afterwards will do nothing in such a case.

> [!NOTE]
>
> Sometimes it may not immediately exit whilst executing a task, if you REALLY need to quite the process you can kill it manually.

### Using Ollama

> If you don't have Ollama installed, you can ignore this section.

If you have Ollama installed already (e.g. via `brew install ollama`) then the launcher script app always use it. Even if the Ollama server is not running, the launcher app will initiate it with `ollama serve` and terminate it when the node is being stopped.

If you would like to explicitly use Docker Ollama instead, you can do this by passing the `--docker-ollama` option.

```sh
# Run with local ollama
./dkn-compute-launcher -m=phi3 --docker-ollama
```

> [!TIP]
>
> There are three Docker Compose Ollama options: `ollama-cpu`, `ollama-cuda`, and `ollama-rocm`. The launcher app will decide which option to use based on the host machine's GPU specifications.

### Additional Static Nodes

You can add additional relay nodes & bootstrap nodes from environment, using the `DKN_RELAY_NODES` and `DKN_BOOTSTRAP_NODES` variables respectively. Simply write the `Multiaddr` string of the static nodes as comma-separated values, and the compute node will pick them up at the start.
