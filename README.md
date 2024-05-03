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
    <a href="https://opensource.org/licenses/MIT" target="_blank">
        <img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-7CB9E8.svg">
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

- **Synthesis**: Using [Ollama](https://github.com/ollama/ollama), nodes will generate synthetic data with respect to prompts given by the admin node.

Each task can be enabled providing the task name as a feature to the executable.

### Waku

We are using a reduced version of [nwaku-compose](https://github.com/waku-org/nwaku-compose) for the Waku node. It only uses the RELAY protocol, and STORE is disabled. The respective files are under the [waku](./waku/) folder.

## Usage

Dria Compute Node is mainly expected to be executed using Docker Compose. The provided compose file will setup everything required. To start running a node, you must do the following:

1. **Prepare Environment Variables**: Dria Compute Node makes use of several environment variables, some of which used by Waku itself as well. First, prepare you environment variable as given in [.env.example](./.env.example).

1. **Fund an Ethereum Wallet with 0.1 Sepolia ETH**: Waku and Dria makes use of the same Ethereum wallet, and Waku uses RLN Relay protocol for further security within the network. If you have not registered to RLN protocol yet, register by running `./register_rln.sh`. If you have already registered, you will have a `keystore.json` which you can place under `./waku/keystore/keystore.json` in this directory.

1.

TODO: describe docker compose

### Run from Source

Clone the repository:

```sh
git clone https://github.com/firstbatchxyz/dkn-compute-node
```

We are using Make as a wrapper for some scripts. You can see the available commands with:

```sh
make help
```

Run Waku and Ollama node elsewhere, and then run the compute node with:

```sh
make run
```

## Testing

Besides the unit tests, there are separate tests for Waku network, and for compute tasks such as Ollama.

```sh
make test         # unit tests
make test-waku    # Waku tests (requires a running Waku node)
make test-ollama  # Ollama tests (requires a running Ollama client)
```

## Styling

Lint and format with:

```sh
make lint
make format
```
