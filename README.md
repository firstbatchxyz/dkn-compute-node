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

## Setup

A Dria Knowledge Node is composed of 3 things:

- [**Compute Node**](https://github.com/firstbatchxyz/dkn-compute-node): basically this repo, handling the computation & interface with Ollama and Waku.
- [**Ollama**](https://github.com/ollama/ollama): locally hosted LLMs
- [**Waku**](https://github.com/waku-org/nwaku-compose): peer-to-peer networking

Using a single Docker Compose file, we have prepared the entire setup, with necessary credentials given via an `.env` file. (TODO)

## Usage

You can run the processes at a set log level:

```sh
RUST_LOG=info cargo run
```

## Testing

Simply run:

```sh
cargo test
```
