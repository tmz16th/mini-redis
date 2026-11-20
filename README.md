# mini-redis

A small key-value store written in Rust, built up in stages. Inspired by [Redis](https://redis.io/).

A key-value store is a dictionary that lives outside your program: other processes (or other machines) can connect to it and ask it to store, retrieve, and delete values by key.

## Current state (stage 3)

- In-memory `HashMap<String, String>` storage
- Append-only write-ahead log (WAL): every write is logged and synced to disk before memory is updated
- Log is replayed on startup, so data survives restarts
- TCP server on `127.0.0.1:6379`
- One thread per client, with the store shared behind `Arc<RwLock<Store>>`
- Simple newline-delimited text protocol, so you can talk to it with `nc`

## Running

```sh
cargo run --release
```

In another terminal:

```sh
nc localhost 6379
```

## Commands

| Command | Example | Response |
| --- | --- | --- |
| `PING` | `PING` | `PONG` |
| `SET` | `SET name Ted` | `OK` |
| `GET` | `GET name` | the value, or `(nil)` |
| `DEL` | `DEL name` | `1` if removed, `0` otherwise |
| `QUIT` | `QUIT` | closes the connection |

Commands are case-insensitive. Values cannot contain spaces yet.

## Testing

```sh
cargo test                                   # correctness
cargo test --release -- --ignored --nocapture # throughput benchmarks
```

The benchmarks report both pure in-memory throughput and WAL throughput with
`fsync` on every write, so the durability cost is measurable.

## Structure

- `src/store.rs` — the storage layer, log replay, and tests. No networking code.
- `src/main.rs` — the TCP server and command parsing.

The log is stored in `mini-redis.wal` in the working directory and is ignored
by git.

Keeping these apart means each layer can be tested and profiled independently.

## Roadmap

- [x] Stage 1 — in-memory store + terminal REPL
- [x] Stage 2 — TCP server with concurrent clients
- [x] Stage 3 — append-only log (WAL) so data survives restarts
- [ ] Stage 4 — snapshots and log compaction
- [ ] Stage 5 — async rewrite with `tokio`
