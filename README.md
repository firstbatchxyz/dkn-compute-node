<p align="center">
  <img src="https://raw.githubusercontent.com/firstbatchxyz/dria-js-client/master/logo.svg" alt="logo" width="142">
</p>

<p align="center">
  <h1 align="center">
    Dria Compute Node
  </h1>
  <p align="center">
    <i>Dria Compute Node serves the computation results within Dria Knowledge Network.</i>
  </p>
</p>

<p align="center">
    <a href="https://opensource.org/license/apache-2-0" target="_blank">
        <img alt="License: Apache-2.0" src="https://img.shields.io/badge/license-Apache%202.0-7CB9E8.svg">
    </a>
    <a href="./.github/workflows/test.yml" target="_blank">
        <img alt="Workflow: Tests" src="https://github.com/firstbatchxyz/dkn-compute-node/actions/workflows/tests.yml/badge.svg?branch=master">
    </a>
    <a href="https://discord.gg/2wuU9ym6fq" target="_blank">
        <img alt="Discord" src="https://dcbadge.vercel.app/api/server/2wuU9ym6fq?style=flat">
    </a>
</p>

## About

A **Dria Compute Node** is a unit of computation within the Dria Knowledge Network. It's purpose is to process tasks given by the **Dria Admin Node**, and receive rewards for providing correct results.

To get started, [setup](#setup) your envrionment and then see [usage](#usage) to run the node.

### Tasks

Compute nodes can technically do any arbitrary task, from computing the square root of a given number to finding LLM outputs from a given prompt, or validating an LLM's output with respect to knowledge available on the web accessed via tools.

- **Ping/Pong**: Dria Admin Node broadcasts **ping** messages at a set interval, it is a required duty of the compute node to respond with a **pong** to these so that they can be included in the list of available nodes for task assignment. These tasks will respect the type of model provided within the pong message, e.g. if a task requires `gpt-4o` and you are running `phi3`, you won't be selected for that task.

- **Workflows**: Each task is given in the form of a workflow, based on [Ollama Workflows](https://github.com/andthattoo/ollama-workflows) (see repository for more information). In simple terms, each workflow defines the agentic behavior of an LLM, all captured in a single JSON file, and can represent things ranging from simple LLM generations to iterative web searching.

## Requirements

The compute node is a very lightweight process, with few MBs of memory usage along with an image size of less than ~65MBs. If you are using Ollama, you will need the memory to run large models locally, which depend on the model's size that you are willing to.

You need the following applications to run compute node:

- **Git**: We will use `git` to clone the repository from GitHub, and pull latest changes for updates later.
- **Docker**: Our services will make use of Docker so that the node can run on any machine.

## Setup

To be able to run a node, we need to make a few simple preparations. Follow the steps below one by one.

### 1. Clone the repository

This repository has the necessary setup to run the node, so start by cloning it using the command below:

```bash
git clone https://github.com/firstbatchxyz/dkn-compute-node
```

### 2. Prepare Environment Variables

Dria Compute Node makes use of several environment variables. Create a `.env` file, and copy the environment variables as given in [.env.example](./.env.example). We will fill out the missing parts in a moment.

```sh
cp .env.example .env
```

> [!NOTE]
>
> `DKN_ADMIN_PUBLIC_KEY` is used to verify that the tasks are given by certain nodes, so that your node does not work for tasks given to the network by untrusted people. You don't need to change this, simply copy and paste it to your `.env`.

### 3. Prepare Ethereum Wallet

Dria makes use of the same Ethereum wallet, that is the recipient of your hard-earned rewards! Place your private key at `DKN_WALLET_SECRET_KEY` in `.env` without the 0x prefix. It should look something like:

```sh
DKN_WALLET_SECRET_KEY=ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

> [!CAUTION]
>
> Always make sure your private key is within the .gitignore'd `.env` file, nowhere else! To be even safer, you can use a throwaway wallet, you can always transfer your rewards to a main wallet afterwards.

### 4. Setup Ollama (for Ollama users)

If you will be using locally served models via Ollama, you must **first pull a small embedding model that is used internally**.

```sh
ollama pull hellord/mxbai-embed-large-v1:f16
```

For the models that you choose (see list of models just below [here](#1-choose-models)) you can download them with same command.

```sh
# example for phi3:3.8b
ollama pull phi3:3.8b
```

Note that if your model size is large, pulling them may take a while.

## Usage

With all setup steps above completed, we are ready to start a node!

### 1. Choose Model(s)

Based on the resources of your machine, you must decide which models that you will be running locally. For example, you can use OpenAI with their models, not running anything locally at all; or you can use Ollama with several models loaded to disk, and only one loaded to memory during its respective task. Available models (see [here](https://github.com/andthattoo/ollama-workflows/blob/main/src/program/atomics.rs#L269) for latest) are:

- `adrienbrault/nous-hermes2theta-llama3-8b:q8_0` (Ollama)
- `phi3:14b-medium-4k-instruct-q4_1` (Ollama)
- `phi3:14b-medium-128k-instruct-q4_1` (Ollama)
- `phi3:3.8b` (Ollama)
- `gpt-3.5-turbo` (OpenAI)
- `gpt-4-turbo` (OpenAI)
- `gpt-4o` (OpenAI)
- `gpt-4o-mini` (OpenAI)

> [!TIP]
>
> If you are using Ollama, make sure you have pulled the required models, as specified in the [section above](#4-setup-ollama-for-ollama-users!

### 2. Start Docker

Our node will be running within a Docker container, so we should make sure that Docker is running before the next step. You can launch Docker via its [desktop application](https://www.docker.com/products/docker-desktop/), or a command such as:

```sh
sudo systemctl start docker
```

> [!NOTE]
>
> You don't need to do this step if Docker is already running in the background.

### 3. Run Node

It's time to run our compute node. We have a starter script that makes this much easier, you can see available commadns with:

See the available commands with:

```sh
chmod +x start.sh
./start.sh --help
```

Simply run the script with the model names provided, such as:

```sh
./start.sh -m=llama3 -m=gpt-3.5-turbo
```

Start script will run the containers in the background. You can check their logs either via the terminal or from [Docker Desktop](https://www.docker.com/products/docker-desktop/).

> [!TIP]
>
> To print DEBUG-level logs for the compute node, you can add `--dev` argument:
>
> ```sh
> ./start.sh -m=<model-name> --dev
> ```

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

When you use `./start.sh`, it will wait for you in the same terminal to do CTRL+C before stopping. Once you do that, the containers will be stopped and removed. You can also kill the containers manually, doing CTRL+C afterwards will do nothing in such a case.

### Using Ollama

> If you don't have Ollama installed, you can ignore this section.

If you have Ollama installed already (e.g. via `brew install ollama`) then you must indicate that you will be using that Ollama, instead of a Docker container. To do this, we set the provide the argument `--local-ollama=true` which is `true` by default. With this, the compute node will use the Ollama server on your machine, instead of a Docker container.

If the Ollama server is not running, the start script will initiate it with `ollama serve` and terminate it when the node is being stopped.

- If `--local-ollama=false` or the local Ollama server is reachable, the compute node will use a Docker Compose service for it.

> [!TIP]
>
> There are three Docker Compose Ollama options: `ollama-cpu`, `ollama-cuda`, and `ollama-rocm`. The start script will decide which option to use based on the host machine's GPU specifications.

```sh
# Run with local ollama
./start.sh -m=phi3 --local-ollama=true
```

### Additional Static Nodes

You can add additional relay nodes & bootstrap nodes from environment, using the `DKN_RELAY_NODES` and `DKN_BOOTSTRAP_NODES` variables respectively. Simply write the `Multiaddr` string of the static nodes as comma-separated values, and the compute node will pick them up at the start.

## Development

> If you have a feature that you would like to add with respect to its respective issue, or a bug fix, feel free to fork & create a PR!

If you would like to run the node from source (which is really handy during development), you can use our shorthand scripts within the Makefile. You can see the available commands with:

```sh
make help
```

You will need OpenSSL installed as well, see shorthand commands [here](https://github.com/sfackler/rust-openssl/issues/855#issuecomment-450057552). While running Ollama elsewhere (if you are using it) or with an OpenAI API key provided, you can run the compute node with:

```sh
make run      # info-level logs
make debug    # debug-level logs
```

### Testing & Benchmarking

You can the tests as follows:

```sh
make test         # unit tests
make test-ollama  # Ollama tests (requires a running Ollama client)
```

To measure the speed of some Ollama models we have a benchmark that uses some models for a few prompts:

```sh
cargo run --release --example ollama
```

You can also benchmark these models using a larger task list at a given path, with the following command:

```sh
JSON_PATH="./path/to/your.json" cargo run --release --example ollama
```

### Documentation

Open crate docs using:

```sh
make docs
```

### Styling

Lint and format with:

```sh
make lint   # clippy
make format # rustfmt
```

## License

This project is licensed under the [Apache License 2.0](https://opensource.org/license/Apache-2.0).
