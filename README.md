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
    <a href="https://hub.docker.com/repository/docker/firstbatch/dkn-compute-node/general" target="_blank">
        <img alt="Docker Version" src="https://img.shields.io/docker/v/firstbatch/dkn-compute-node?logo=Docker&label=image&color=2496ED&sort=semver">
    </a>
    <a href="https://discord.gg/dria" target="_blank">
        <img alt="Discord" src="https://dcbadge.vercel.app/api/server/dria?style=flat">
    </a>
</p>

## About

A **Dria Compute Node** is a unit of computation within the Dria Knowledge Network. It's purpose is to process tasks given by the **Dria Admin Node**. To get started, see [node guide](./docs/NODE_GUIDE.md)!

### Tasks

Compute nodes can technically do any arbitrary task, from computing the square root of a given number to finding LLM outputs from a given prompt, or validating an LLM's output with respect to knowledge available on the web accessed via tools.

- **Ping/Pong**: Dria Admin Node broadcasts **ping** messages at a set interval, it is a required duty of the compute node to respond with a **pong** to these so that they can be included in the list of available nodes for task assignment. These tasks will respect the type of model provided within the pong message, e.g. if a task requires `gpt-4o` and you are running `phi3`, you won't be selected for that task.

- **Workflows**: Each task is given in the form of a workflow, based on [Ollama Workflows](https://github.com/andthattoo/ollama-workflows) (see repository for more information). In simple terms, each workflow defines the agentic behavior of an LLM, all captured in a single JSON file, and can represent things ranging from simple LLM generations to iterative web searching.

## Node Running

Refer to [node guide](./docs/NODE_GUIDE.md) to quickly get started and run your own node!

## Releases

For _production_ images:

- **Versioned**: With each release, a versioned image is deployed on Docker hub with the version tag `:vX.X.X`.
- **Latest**: The latest production image is always under the `:latest` tag.

For _development_ images:

- **Master**: On each push to `master` branch, a new image is created with the tag `master-<commit>-<timestamp>`.
- **Unstable**: The latest development image is always under the `:unstable` tag.

You can see the list of deployed images on [Docker Hub](https://hub.docker.com/orgs/firstbatch/members).

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

You can run the latest Docker image via compose as well:

```sh
docker compose up

# Ollama without any GPUs
docker compose --profile=ollama-cpu up
# Ollama for NVIDIA gpus
docker compose --profile=ollama-cuda up
# Ollama for AMD gpus
docker compose --profile=ollama-rocm up
```

### Testing

You can the tests as follows:

```sh
make test
```

We also have some benchmarking and profiling scripts, see [node performance](./docs/NODE_PERFORMANCE.md) for more details.

### Documentation

You can view the inline documentation with:

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
