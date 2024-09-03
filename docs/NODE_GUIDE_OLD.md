## Node Running

Running a Dria Compute Node is pretty straightforward.

## Requirements

### Software

You need the following applications to run compute node:

- **Git**: We will use `git` to clone the repository from GitHub, and pull latest changes for updates later.
- **Docker**: Our services will make use of Docker so that the node can run on any machine.

> [!CAUTION]
>
> In **windows** machines, Docker-Desktop requried to be running with **WSL2**
>
> You can check the Docker Desktop Windows installation guide from [here](https://docs.docker.com/desktop/install/windows-install/)

> [!TIP]
>
> You can check if you have these via:
>
> ```sh
> which git
> which docker
> ```

### Hardware

**To learn about hardware specifications such as required CPU and RAM, please refer to [node specifications](./docs/NODE_SPECS.md).**

In general, if you are using Ollama you will need the memory to run large models locally, which depend on the model's size that you are willing to. If you are in a memory-constrained environment, you can opt to use OpenAI models instead.

> [!NOTE]
>
> The compute node is a lightweight process, but you may see increased memory & CPU usage during the initial testing phases, due to various protocol-level operations with the growing network size.

## Setup

To be able to run a node, we need to make a few simple preparations. Follow the steps below one by one.

### 1. Download and Extract the Node Files

Download the appropriate ZIP file for your system using the commands below or from [browser](https://github.com/firstbatchxyz/dkn-compute-node/releases/tag/untagged-ad4b77bd5544f4412179). Make sure to replace the URL with the correct version for your operating system and architecture.

**macOS:**

1. Check your architecture:

   ```sh
   uname -m
   ```

   - If the output is `arm64`, download the `arm64` version.
   - If it's `x86_64`, download the `amd64` version.

2. Download the ZIP file:

   ```sh
   curl -L -o dkn-compute-node.zip https://github.com/firstbatchxyz/dkn-compute-node/releases/download/untagged-54b9278cc672499d9c84/dkn-compute-node-macos-arm64.zip
   ```

3. Unzip the downloaded file:
   ```sh
   unzip dkn-compute-node-macos-arm64.zip
   cd dkn-compute-node-darwin-arm64
   ```

> [!TIP]
>
> Some devices need you to bypass macOS's security warning. If you see "macOS cannot verify that this app is free from malware," use the following command:
>
> ```sh
> xattr -d com.apple.quarantine start
> ```

**Linux:**

1. Check your architecture:

   ```sh
   uname -m
   ```

   - If the output is `x86_64`, download the `amd64` version.
   - If it's `aarch64`, download the `arm64` version.

2. Download the ZIP file:

   ```sh
   curl -L -o dkn-compute-node.zip https://github.com/firstbatchxyz/dkn-compute-node/releases/download/untagged-54b9278cc672499d9c84/dkn-compute-node-linux-amd64.zip
   ```

3. Unzip the downloaded file:
   ```sh
   unzip dkn-compute-node-linux-amd64.zip
   cd dkn-compute-node-linux-amd64
   ```

**Windows:**

1. Check your architecture:

   - Open System Information:
     - Press `Win + R` to open the Run dialog.
     - Type `msinfo32` and press Enter.
   - Look for the line labeled "Processor" or "CPU":
     - If it includes "x64" or refers to Intel or AMD, it is likely x86 (amd64).
     - If it mentions ARM, then it's an ARM processor.

2. Download the ZIP file using a web browser or in PowerShell:

   ```cmd
   curl -L -o dkn-compute-node.zip https://github.com/firstbatchxyz/dkn-compute-node/releases/download/untagged-54b9278cc672499d9c84/dkn-compute-node-windows-amd64.zip
   ```

3. Unzip the downloaded file using File Explorer or in PowerShell:
   ```cmd
   Expand-Archive -Path "C:\path\to\your\dkn-compute-node-windows-amd64.zip" -DestinationPath "C:\path\to\destination\dkn-compute-node-windows-amd64"
   cd dkn-compute-node-windows-amd64
   ```

### 2. Prepare Environment Variables

> [!TIP]
>
> Speed-running the node execution:
>
> Optionally, you can also handle the environment variables on the fly by just running the `start` cli-app directly, since it'll ask you to enter the required environment variables.
>
> If you prefer this you can move on to the [Usage](#usage) section

Dria Compute Node makes use of several environment variables. Create a `.env` file, and copy the environment variables as given in [.env.example](./.env.example). We will fill out the missing parts in a moment.

```sh
cp .env.example .env
```

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

Dria makes use of the same Ethereum wallet, that is the recipient of your hard-earned rewards! Place your private key at `DKN_WALLET_SECRET_KEY` in `.env` without the 0x prefix. It should look something like:

```sh
DKN_WALLET_SECRET_KEY=ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

> [!CAUTION]
>
> Always make sure your private key is within the .gitignore'd `.env` file, nowhere else! To be even safer, you can use a throwaway wallet, you can always transfer your rewards to a main wallet afterwards.

### 4. Setup LLM Provider

For the final step, we need to make sure we can serve LLM requests.

#### For OpenAI

If you will be using OpenAI to serve its models, you need to have an API key in the environment. Simply set the key within your `.env`:

```sh
OPENAI_API_KEY=<YOUR_KEY>
```

#### For Ollama

Of course, first you have to install Ollama; see their [download page](https://ollama.com/download). Then, you must **first pull a small embedding model that is used internally**.

```sh
ollama pull hellord/mxbai-embed-large-v1:f16
```

For the models that you choose (see list of models just below [here](#1-choose-models)) you can download them with same command. Note that if your model size is large, pulling them may take a while.

```sh
# example for phi3:3.8b
ollama pull phi3:3.8b
```

> [!TIP]
>
> Alternatively, you can set `OLLAMA_AUTO_PULL=true` in the `.env` so that the compute node will always download the missing models for you.

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
- `phi3:3.8b`
- `llama3.1:latest`
- `llama3.1:8b-instruct-q8_0`
- `phi3.5:3.8b`
- `phi3.5:3.8b-mini-instruct-fp16`

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

It's time to run our compute node. We have a starter cli app that makes this much easier, you can either run it by double-clicking the `start` app (`start.exe` on windows) from your file explorer or use it from terminal (or cmd/powershell in windows).

See the available commands with:

```sh
# macos or linux
./start --help

# windows
.\start.exe --help
```

Then simply run the cli app, it will ask you to enter required inputs:

```sh
# macos or linux
./start

# windows
.\start.exe
```

Or you can directly pass the running models using `-m` flags

```sh
# macos or linux
./start -m=llama3.1:latest -m=gpt-3.5-turbo

# windows
.\start.exe -m=llama3.1:latest -m=gpt-3.5-turbo
```

Start app will run the containers in the background. You can check their logs either via the terminal or from [Docker Desktop](https://www.docker.com/products/docker-desktop/).

#### Running in Debug Mode

To print DEBUG-level logs for the compute node, you can add `--dev` argument to the start app. For example:

```sh
./start -m=gpt-4o-mini --dev
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

When you start your node with `start` cli app, it will wait for you in the same terminal to do CTRL+C before stopping. Once you do that, the containers will be stopped and removed. You can also kill the containers manually, doing CTRL+C afterwards will do nothing in such a case.

> [!NOTE]
>
> Sometimes it may not immediately exit whilst executing a task, if you REALLY need to quite the process you can kill it manually.

### Using Ollama

> If you don't have Ollama installed, you can ignore this section.

If you have Ollama installed already (e.g. via `brew install ollama`) then you must indicate that you will be using that Ollama, instead of a Docker container. To do this, we set the provide the argument `--local-ollama=true` which is `true` by default. With this, the compute node will use the Ollama server on your machine, instead of a Docker container.

If the Ollama server is not running, the start app will initiate it with `ollama serve` and terminate it when the node is being stopped.

- If `--local-ollama=false` or the local Ollama server is reachable, the compute node will use a Docker Compose service for it.

> [!TIP]
>
> There are three Docker Compose Ollama options: `ollama-cpu`, `ollama-cuda`, and `ollama-rocm`. The start app will decide which option to use based on the host machine's GPU specifications.

```sh
# Run with local ollama
./start -m=phi3 --local-ollama=true
```

### Additional Static Nodes

You can add additional relay nodes & bootstrap nodes from environment, using the `DKN_RELAY_NODES` and `DKN_BOOTSTRAP_NODES` variables respectively. Simply write the `Multiaddr` string of the static nodes as comma-separated values, and the compute node will pick them up at the start.
