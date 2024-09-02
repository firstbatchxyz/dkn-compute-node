# Node Performance

We have some benchmarks to see model performance using Ollama, and some profiling scripts to check CPU and memory usage.

## Benchmarking

You can the tests as follows:

```sh
make test         # unit tests
```

To measure the speed of some Ollama models we have a benchmark that uses some models for a few prompts:

```sh
cargo run --release --example ollama
```

You can also benchmark these models using a larger task list at a given path, with the following command:

```sh
JSON_PATH="./path/to/your.json" cargo run --release --example ollama
```

## Profiling

We have scripts to profile both CPU and Memory usage. A special build is created for profiling, via a custom `profiling` feature, such that the output inherits `release` mode but also has debug symbols.

Furthermore, the profiling build will exit automatically after a certain time, as if CTRL+C has been pressed. This is needed by the memory profiling tool in particular.

### CPU Profiling

To create a [flamegraph](https://crates.io/crates/flamegraph) of the application, do:

```sh
make profile-cpu
```

This will create a profiling build that inherits `release` mode, except with debug information.

> [!NOTE]
>
> CPU profiling may require super-user access.

### Memory Profiling

To profile memory usage, we make use of [cargo-instruments](https://crates.io/crates/cargo-instruments).

```sh
make profile-mem
```
