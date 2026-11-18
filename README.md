# mini-redis

A small key-value store written in Rust, built up in stages. Inspired by [Redis](https://redis.io/).

A key-value store is a dictionary that lives outside your program: other processes (or other machines) can connect to it and ask it to store, retrieve, and delete values by key.

## Current state (stage 1)

- In-memory `HashMap<String, String>` storage
- Terminal REPL for poking at the store

## Running

```sh
cargo run
```

Then try `SET name Ted`, `GET name`, `DEL name`, `EXIT`.

## Testing

```sh
cargo test
cargo test --release -- --ignored --nocapture # throughput benchmark
```

## Structure

- `src/store.rs` — the storage layer and its tests. No interface code.
- `src/main.rs` — the terminal interface.

Keeping these apart means each layer can be tested and profiled independently.

## Roadmap

- [x] Stage 1 — in-memory store + terminal REPL
- [ ] Stage 2 — TCP server with concurrent clients
- [ ] Stage 3 — append-only log (WAL) so data survives restarts
- [ ] Stage 4 — snapshots and log compaction
- [ ] Stage 5 — async rewrite with `tokio`
