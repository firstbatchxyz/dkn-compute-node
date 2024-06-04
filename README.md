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

### Heartbeat

Dria Admin Node broadcasts heartbeat messages at a set interval, it is a required duty of the compute node to respond to these so that they can be included in the list of available nodes for task assignment.

### Tasks

Compute nodes can technically do any arbitrary task, from computing the square root of a given number to finding LLM outputs from a given prompt. We currently have the following tasks:

- **Synthesis**: Generate synthetic data with respect to prompts given by the admin node.
- **Search**: Search the web using reasoning-and-action to answer a given query.
- **Validation**: Validate a given query-response pair. _(WIP)_

Tasks are enabled or disabled via the `DKN_TASKS` environment variable. Task names are to be provided in a list of comma-separated strings such as `DKN_TASKS=synthesis,search`.

### Waku

We are using a reduced version of [nwaku-compose](https://github.com/waku-org/nwaku-compose) for the Waku node. It only uses the RELAY protocol, and STORE is disabled. The respective files are under the [waku](./waku/) folder.

By default, there are no static peers, but you can specify them using duplicate `--staticnode` arguments within the `EXTRA_ARGS` variable which is passed to the Waku node, that is:

```sh
EXTRA_ARGS="--staticnode=/ip4/foobar/... --staticnode=/ip4/bazboo/..."
```

## Usage

Dria Compute Node is mainly expected to be executed using Docker Compose. The provided compose file will setup everything required. To start running a node, you must do the following:

1. **Prepare Environment Variables**: Dria Compute Node makes use of several environment variables, some of which used by Waku itself as well. First, prepare you environment variable as given in [.env.example](./.env.example).

1. **Fund an Ethereum Wallet with 0.1 Sepolia ETH**: Waku and Dria makes use of the same Ethereum wallet, and Waku uses RLN Relay protocol for further security within the network. If you have not registered to RLN protocol yet, register by running `./register_rln.sh`. If you have already registered, you will have a `keystore.json` which you can place under `./waku/keystore/keystore.json` in this directory. Your secret key will be provided at `ETH_TESTNET_KEY` variable. You can set an optional password at `RLN_RELAY_CRED_PASSWORD` as well to encrypt the keystore file, or to decrypt it if you already have one.

1. **Ethereum Client RPC**: To communicate with Sepolia, you need an RPC URL. You can use [Infura](https://app.infura.io/) or [Alchemy](https://www.alchemy.com/). Your URL will be provided at `ETH_CLIENT_ADDRESS` variable.

With all of these steps completed, you should be able to start a node with:

```sh
# clone the repo
git clone https://github.com/firstbatchxyz/dkn-compute-node

# -d to run in background
docker compose up -d
```

With `-d` option, the containers will be running in the background. You can check their logs either via the terminal or from [Docker Desktop](https://www.docker.com/products/docker-desktop/).

### Ollama Configuration

You have several alternatives to use Ollama:

- `docker compose --profile ollama-cpu up -d` will launch Ollama container using CPU only.
- `docker compose --profile ollama-cuda up -d` will launch Ollama container with CUDA support, for NVIDIA gpus.
- `docker compose --profile ollama-rocm up -d` will launch Ollama container with ROCM support, for AMD gpus.
- For Apple Silicon, you must install Ollama (e.g. `brew install ollama`) and launch the server (`ollama serve`) in another terminal, and then simply `docker compose up -d`.

You can decide on a model to use by changing `OLLAMA_MODEL` variable, such as `OLLAMA_MODEL=llama3`. See [Ollama library](https://ollama.com/library) for the catalog of models.

## Run from Source

We are using Make as a wrapper for some scripts. You can see the available commands with:

```sh
make help
```

You will need OpenSSL installed as well, see shorthand commands [here](https://github.com/sfackler/rust-openssl/issues/855#issuecomment-450057552).

### Running Compute Node

While running Waku and Ollama node elsewhere, you can run the compute node with:

```sh
make run      # info-level logs
make debug    # debug-level logs
```

## Docs

Open crate docs using:

```sh
make docs
```

## Testing

Besides the unit tests, there are separate tests for Waku network, and for compute tasks such as Ollama.

```sh
make test         # unit tests
make test-waku    # Waku tests (requires a running Waku node)
make test-ollama  # Ollama tests (requires a running Ollama client)
```

## Benchmarking

To measure the speed of some Ollama models we have a benchmark that uses some models for a few prompts:

```sh
cargo run --release --example ollama
```

You can also benchmark these models using a larger task list at a given path, with the following command:

```sh
JSON_PATH="./path/to/your.json" cargo run --release --example ollama
```

## Styling

Lint and format with:

```sh
make lint   # clippy
make format # rustfmt
```
