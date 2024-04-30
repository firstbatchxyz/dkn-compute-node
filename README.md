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

## About

A **Dria Compute Node** is a unit of computation within the Dria Knowledge Network. It's purpose is to process tasks given by the **Dria Admin Node**, and receive rewards for providing correct results. These nodes are part of the [Waku](https://waku.org/) network, a privacy-preserving cencorship resistant peer-to-peer network.

### Heartbeat

Dria Admin Node broadcasts heartbeat messages at a set interval, it is a required duty of the compute node to respond to these so that they can be included in the list of available nodes for task assignment.

### Tasks

Compute nodes can technically do any arbitrary task, from computing the square root of a given number to finding LLM outputs from a given prompt. We currently have the following tasks:

- **Synthesis**: Using [Ollama](https://github.com/ollama/ollama), nodes will generate synthetic data with respect to prompts given by the admin node.

Each task can be enabled providing the task name as a feature to the executable.

## Usage with Compose

TODO: describe docker compose

## Usage from Source

We are using [Just](https://just.systems/) as a wrapper for some scripts. You can see the available commands with:

```sh
just -l
```

## Styling

Lint and format with:

```sh
just lint
just format
```

## Testing

Besides the unit tests, there are separate tests for Waku network, and for compute tasks such as Ollama.

```sh
just test         # unit tests
just test-waku    # Waku tests (requires a running Waku node)
just test-ollama  # Ollama tests (requires a running Ollama client)
```
