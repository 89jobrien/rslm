# rslm demo

Demonstrates the three core RLM capabilities against the
[mini-redis](https://github.com/tokio-rs/mini-redis) source code (~1400 lines, 7 modules).

| Demo | Capability | Query |
|------|-----------|-------|
| 1 | `ctx_grep` — keyword lookup | How does mini-redis limit concurrent connections? |
| 2 | `ctx_slice` + multi-step | How does `Db` handle key expiration — data structures + background task? |
| 3 | `rlm_call` — recursive delegation | Compare server shutdown vs. Db shutdown — signals and wait conditions |

## Prerequisites

Set at least one of:

```sh
export OPENAI_API_KEY=<key>       # default provider
export ANTHROPIC_API_KEY=<key>    # use with --provider anthropic
```

## Run the full demo

```sh
# From the repo root — OpenAI (default)
nu docs/demo/demo.nu

# Anthropic
nu docs/demo/demo.nu --provider anthropic

# Specific model
nu docs/demo/demo.nu --provider openai --model gpt-4o-mini
```

## Run a single query manually

```sh
cargo run --bin rslm -- --verbose query \
  "How does the Connection struct buffer frames from the TCP stream?" \
  --context-file docs/demo/context.txt
```

## What to look for

With `--verbose`, each Rhai cell prints as it executes:

```
[depth=0] >> ctx_grep("Semaphore")
[depth=0] << limit_connections: Arc<Semaphore>,
             let permit = self.limit_connections.clone().acquire_owned()...
[depth=0] >> final_answer("mini-redis uses a Semaphore with MAX_CONNECTIONS = 250...")
[depth=0] FINAL: mini-redis uses a Semaphore...
```

For Demo 3, watch for `[depth=1]` lines — child RLM instances spawned by `rlm_call` to
answer sub-questions against focused context slices, with their answers bubbled back up.

## Context file

`context.txt` is the concatenated source of 7 mini-redis modules fetched from
`tokio-rs/mini-redis` master: `lib.rs`, `server.rs`, `connection.rs`, `db.rs`, `frame.rs`,
`shutdown.rs`. License: MIT OR Apache-2.0.
