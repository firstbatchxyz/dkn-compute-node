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

A **Dria Compute Node** is a unit of computation within the Dria Knowledge Network. It's purpose is to process tasks given by the **Dria Admin Node**, and receive rewards for providing correct results. These nodes are part of the [Waku](https://waku.org/) network, a privacy-preserving cencorship resistant peer-to-peer network.

To get started, [setup](#setup) your envrionment and then see [usage](#usage) to run the node.

### Tasks

Compute nodes can technically do any arbitrary task, from computing the square root of a given number to finding LLM outputs from a given prompt, or validating an LLM's output with respect to knowledge available on the web accessed via tools.

- **Heartbeats**: Dria Admin Node broadcasts **heartbeat** messages at a set interval, it is a required duty of the compute node to respond to these so that they can be included in the list of available nodes for task assignment. These tasks will respect the type of model provided, e.g. if a task requires `gpt-4o` and you are running `phi3`, you won't be selected for that task.

- **Workflows**: Each task is given in the form of a workflow, based on [Ollama Workflows](https://github.com/andthattoo/ollama-workflows) (see repository for more information). In simple terms, each workflow defines the agentic behavior of an LLM, all captured in a single JSON file, and can represent things ranging from simple LLM generations to iterative web searching.

### Waku

We are using a reduced version of [nwaku-compose](https://github.com/waku-org/nwaku-compose) for the Waku node. It only uses the Relay protocol, and Store is disabled. The respective files are under the [waku](./waku/) folder.

By default, there are no static peers, but you can specify them using duplicate `--staticnode` arguments within the `WAKU_EXTRA_ARGS` variable which is passed to the Waku node, that is:

```sh
WAKU_EXTRA_ARGS="--staticnode=/ip4/foobar/... --staticnode=/ip4/bazboo/..."
```

## Requirements

Your machine should have **at least 2GB** memory, along with a stable internet connection.

You need the following applications to run compute node:

- **Git**: We will use `git` to clone the repository from GitHub, and pull latest changes for updates later.
- **Docker**: Our services will make use of Docker so that the node can run on any machine.

## Setup

To be able to run a node, we need to make a few preparations. Follow the steps below one by one.

> [!TIP]
>
> These setup steps are all to be able to use Waku network. You can find a similar setup under [nwaku-compose](https://github.com/waku-org/nwaku-compose/) as well.

### 1. Clone the repository

This repository has the necessary setup to run the node, so start by cloning it using the command below:

```bash
git clone https://github.com/firstbatchxyz/dkn-compute-node
```

### 2. Prepare Environment Variables

Dria Compute Node makes use of several environment variables, some of which used by Waku itself as well. Create a `.env` file, and prepare you environment variables as given in [.env.example](./.env.example).

```sh
cp .env.example .env
```

### 3. Prepare Ethereum Wallet

Waku and Dria makes use of the same Ethereum wallet. In particular, we require a bit of **testnet ether** (0.1 ETH + gas fees) for the next step, so you should fund your wallet using a faucet such as [Infura](https://www.infura.io/faucet/sepolia) or [Alchemy](https://www.alchemy.com/faucets/ethereum-sepolia).

Place your private key at `ETH_TESTNET_KEY` in `.env` without the 0x prefix. It should look something like:

```sh
ETH_TESTNET_KEY=ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

### 4. Prepare Ethereum Client RPC URL

To communicate with Ethereum, you need an RPC URL. You can use [Infura](https://app.infura.io/) or [Alchemy](https://www.alchemy.com/) providers for this.

Place your URL at the `RLN_RELAY_ETH_CLIENT_ADDRESS` variable in `.env`. It should look something like one of the below:

```sh
# infura
RLN_RELAY_ETH_CLIENT_ADDRESS=https://sepolia.infura.io/v3/<your-key-here>
# alchemy
RLN_RELAY_ETH_CLIENT_ADDRESS=https://eth-sepolia.g.alchemy.com/v2/<your-key-here>
```

### 5. Register to RLN Contract

Waku uses Rate-Limiting Nullifier (RLN) for further security within the network. To register your wallet to RLN, first set a password at `RLN_RELAY_CRED_PASSWORD`. Then, register with the following commands which will create a file at `./waku/keystore/keystore.json`.

```sh
cd waku
./register.rln
```

If all goes well, you should be able to see your transaction at the block explorer under the [RLN contract](https://sepolia.etherscan.io/address/0xCB33Aa5B38d79E3D9Fa8B10afF38AA201399a7e3).

> [!TIP]
>
> If you have already registered before, you will have a `keystore.json` which you can place under `./waku/keystore/keystore.json` in this directory. Note that the private key and RLN password must be the same so that this keystore file can be decrypted.

## Usage

With all setup steps above completed, we are ready to start a node! See the available commands with:

```sh
chmod +x start.sh
./start.sh --help
```

### Choose a Model

Based on the resources of your machine, you must decide which models that you will be running locally. For example, you can use OpenAI with their models, not running anything locally at all; or you can use Ollama with several models loaded to disk, and only one loaded to memory during its respective task. Available models (see [here](https://github.com/andthattoo/ollama-workflows/blob/main/src/program/atomics.rs#L269) for latest) are:

- `adrienbrault/nous-hermes2theta-llama3-8b:q8_0` (Ollama)
- `phi3:14b-medium-4k-instruct-q4_1` (Ollama)
- `phi3:14b-medium-128k-instruct-q4_1` (Ollama)
- `phi3:3.8b` (Ollama)
- `gpt-3.5-turbo` (OpenAI)
- `gpt-4-turbo` (OpenAI)
- `gpt-4o` (OpenAI)

### Run Node

It's time to run our compute node. After deciding the models that you want to run, simply run `./start.sh` with the model names provided, such as:

```sh
./start.sh -m=llama3 -m=gpt-3.5-turbo
```

Start script will run the containers in the background. You can check their logs either via the terminal or from [Docker Desktop](https://www.docker.com/products/docker-desktop/). To print DEBUG-level logs for the compute node, you can add `--dev` argument:

```sh
./start.sh -m=<model-name> --dev
```

### Persistent Waku

To persist your Waku session between runs, you can opt to run Waku elsewhere (such as with [nwaku-compose](https://github.com/waku-org/nwaku-compose/)) and then have the compute node connect to the existing Waku node. For such cases, we have `--waku-ext` flag (meaning Waku is externally hosted):

```sh
./start.sh -m=<model-name> --waku-ext
```

### Using Ollama

> If you don't have Ollama installed, you can ignore this section.

If you have Ollama installed already (e.g. via `brew install ollama`) then you must indicate that you will be using that Ollama, instead of a Docker container.

To do this, we set the provide the argument `--local-ollama=true` which is `true` by default. With this, the compute node will use the Ollama server on your machine, instead of a Docker container.

If the Ollama server is not running, the start script will initiate it with `ollama serve` and terminate it when the node is being stopped.

- If `--local-ollama=false` or the local Ollama server is reachable, the compute node will use a Docker Compose service for it.

> [!TIP]
>
> There are three Docker Compose Ollama options: `ollama-cpu`, `ollama-cuda`, and `ollama-rocm`. The start script will decide which option to use based on the host machine's GPU specifications.

```sh
# Run with local ollama
./start.sh -m=phi3 --local-ollama=true
```

### Run from Source

We are using Make as a wrapper for some scripts. You can see the available commands with:

```sh
make help
```

You will need OpenSSL installed as well, see shorthand commands [here](https://github.com/sfackler/rust-openssl/issues/855#issuecomment-450057552). While running Waku and Ollama node elsewhere, you can run the compute node with:

```sh
make run      # info-level logs
make debug    # debug-level logs
```

## Contributing

If you have a feature that you would like to add with respect to its respective issue, or a bug fix, feel free to fork & create a PR! See the sections below for development tips.

### Testing & Benchmarking

Besides the unit tests, there are separate tests for Waku network, and for compute tasks such as Ollama.

```sh
make test         # unit tests
make test-waku    # Waku tests (requires a running Waku node)
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
