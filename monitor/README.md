# Dria Knowledge Network Monitor

The monitor node generates a random peer ID, and listens to task messages only. It does not process them or respond to any heartbeat requests. It keeps track of `task` and `result` messages, and prints the "pending" tasks at specific intervals.

## Usage

To run:

```sh
cargo run --bin dkn-monitor
```

You can do CTRL+C to terminate the node.
