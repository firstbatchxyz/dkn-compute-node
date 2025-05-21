<p align="center">
  <img src="https://raw.githubusercontent.com/firstbatchxyz/.github/refs/heads/master/branding/dria-logo-square.svg" alt="logo" width="168">
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
    <a href="https://github.com/firstbatchxyz/dkn-compute-node/releases" target="_blank">
        <img alt="Downloads" src="https://img.shields.io/github/downloads/firstbatchxyz/dkn-compute-node/total?logo=github&logoColor=%23F2FFEE&color=%2332C754">
    </a>
    <a href="https://hub.docker.com/repository/docker/firstbatch/dkn-compute-node/general" target="_blank">
        <img alt="Docker Version" src="https://img.shields.io/docker/v/firstbatch/dkn-compute-node?logo=Docker&label=image&color=2496ED&sort=semver">
    </a>
    <a href="https://discord.gg/dria" target="_blank">
        <img alt="Discord" src="https://dcbadge.vercel.app/api/server/dria?style=flat">
    </a>
</p>

> Use the [Dria Compute Launcher](https://github.com/firstbatchxyz/dkn-compute-launcher/) to run a compute node with many more features!

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

You can run the binary as is:

```sh
cargo run

# specify custom .env file
DKN_COMPUTE_ENV=./path/to/.env cargo run
```

If you have a valid `.env` file, you can run the latest Docker image via compose as well:

```sh
docker compose up

# Ollama without any GPUs
docker compose --profile=ollama-cpu up
# Ollama for NVIDIA gpus
docker compose --profile=ollama-cuda up
# Ollama for AMD gpus
docker compose --profile=ollama-rocm up
```

> [!TIP]
>
> You can specify a custom initial RPC address with `DKN_INITIAL_RPC_ADDR`.

### Testing

You can the tests as follows:

```sh
cargo test --workspace
```

We also have some benchmarking and profiling scripts, see [node performance](./docs/NODE_PERFORMANCE.md) for more details.

### Documentation

You can view the entire crate-level documentation with:

```sh
cargo doc --open --no-deps --document-private-items
```

### Styling

Lint and format with:

```sh
cargo clippy --workspace
cargo fmt -v
```

### Profiling

We have scripts to profile both CPU and Memory usage. A special build is created for profiling, via a custom `profiling` feature, such that the output inherits `release` mode but also has debug symbols.

Furthermore, the profiling build will exit automatically after a certain time, as if CTRL+C has been pressed. This is needed by the memory profiling tool in particular.

**CPU Profiling**: To create a [flamegraph](https://crates.io/crates/flamegraph) of the application, the command below will create a profiling build that inherits `release` mode, except with debug information:

```sh
DKN_EXIT_TIMEOUT=120 cargo flamegraph --root --profile=profiling --bin dkn-compute
```

> [!NOTE]
>
> CPU profiling may require super-user access.

**Memory Profiling**: To profile memory usage, we make use of [cargo-instruments](https://crates.io/crates/cargo-instruments):

```sh
DKN_EXIT_TIMEOUT=120 cargo instruments --profile=profiling -t Allocations --bin dkn-compute
```

> [!TIP]
>
> You can adjust the profiling duration via the `DKN_EXIT_TIMEOUT` variable, which takes a number of seconds until termination.

## License

This project is licensed under the [Apache License 2.0](https://opensource.org/license/Apache-2.0).
