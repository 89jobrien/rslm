# rslm

<!-- markdownlint-disable MD013 -->

`rslm` is an experimental Rust implementation of Recursive Language Model (RLM) inference. Rather
than placing an entire context in each model request, it asks a model to produce Rhai scripts that
inspect the context through registered functions, optionally delegate focused subproblems to child
RLM instances, and eventually call `final_answer`.

The raw context is not inserted directly into provider messages. Script results, including excerpts
returned by context functions, are fed back to the model as the loop progresses. Every script runs
with a fresh Rhai `Scope`, so variables do not persist between iterations.

## Contents

- [Status](#status)
- [How it works](#how-it-works)
- [Workspace layout](#workspace-layout)
- [Build and install](#build-and-install)
- [Provider configuration](#provider-configuration)
- [CLI quickstart](#cli-quickstart)
- [Chunk store and retrieval](#chunk-store-and-retrieval)
- [Rust API](#rust-api)
- [Cargo features](#cargo-features)
- [Development and testing](#development-and-testing)
- [Documentation](#documentation)

## Status

The workspace is pre-release software at version `0.1.0`. The recursive loop, OpenAI and Anthropic
providers, CLI, SQLite chunk store, BM25/semantic hybrid search, deterministic test harness, demos,
and benchmark crate are implemented. APIs, prompts, defaults, and storage formats may still change.

Rhai limits each script to 1,000,000 operations, but this project should not be treated as a general
OS-level sandbox for arbitrary untrusted code.

## How It Works

```text
query + provider
       |
       v
model returns Rhai script
       |
       v
fresh Rhai scope reads context through ctx_* functions
       |
       +---- rlm_call(query, sub_context) -> child RLM
       |
       +---- cell output -> next model turn
       |
       `---- final_answer(text) -> caller
```

The core Rhai environment registers:

| Function | Purpose |
| --- | --- |
| `ctx_len()` | Return context length in bytes |
| `ctx_slice(start, end)` | Read a byte range, adjusted to UTF-8 boundaries |
| `ctx_grep(pattern)` | Return context lines matching a Rust regular expression |
| `print_cell(message)` | Add text to the current cell output |
| `rlm_call(query, context)` | Run a child RLM against a focused sub-context |
| `final_answer(answer)` | Finish the loop and return an answer |

With the store enabled, scripts also receive these chunk-level retrieval functions:

| Rhai signature | Purpose |
| --- | --- |
| `doc_id() -> String` | Return the configured document ID |
| `ctx_chunks(doc_id: String) -> int` | Return the document's chunk count |
| `ctx_chunk(doc_id: String, index: int) -> String` | Return one chunk or an empty string |
| `ctx_search(doc_id: String, query: String, k: int) -> String` | Run BM25 keyword search |
| `ctx_hybrid(doc_id: String, query: String, k: int) -> String` | Run hybrid or BM25 search |

## Workspace Layout

| Package | Responsibility |
| --- | --- |
| `rslm-core` | Recursive loop, Rhai environment, protocol types, and optional store integration |
| `rslm-providers` | Provider trait plus OpenAI and Anthropic adapters |
| `rslm-cli` | `query`, `interactive`, `ingest`, and Nushell completion entry points |
| `rslm-store` | SQLite chunks, chunking, BM25, embeddings, HNSW, and RRF hybrid search |
| `rslm-harness` | Scripted provider and deterministic RLM conformance harness |
| `rslm-bench` | Criterion performance benches and deterministic accuracy evaluation |

## Build And Install

The workspace uses Rust 2021.

```sh
git clone https://github.com/89jobrien/rslm.git
cd rslm
cargo build --workspace
cargo install --path crates/rslm-cli
```

Run from the checkout without installing:

```sh
cargo run -p rslm-cli -- --help
```

## Provider Configuration

Set the key for the selected provider:

```sh
export OPENAI_API_KEY=<key>
# or
export ANTHROPIC_API_KEY=<key>
```

Provider and model selection can come from flags or environment variables:

| Setting | Default |
| --- | --- |
| `--provider` / `RSLM_PROVIDER` | `openai` |
| `--model` / `RSLM_MODEL` | `gpt-4o` for OpenAI; `claude-sonnet-4-6` for Anthropic |
| `--max-depth` | `5` |
| `--max-iterations` | `20` |
| `--embed-model` | `text-embedding-3-small` |
| `RUST_LOG` | Standard `tracing-subscriber` filter; unset by default |

Command-line flags take precedence over environment variables. `--verbose` writes generated Rhai
scripts, cell outputs, recursion depth, and final answers to stdout.

## CLI Quickstart

Query a context file:

```sh
cargo run -p rslm-cli -- query \
  "How does the server limit concurrent connections?" \
  --context-file docs/demo/context.txt \
  --verbose
```

Use an inline context or another provider:

```sh
cargo run -p rslm-cli -- \
  --provider anthropic \
  query "What color is the object?" \
  --context "The object is blue."
```

Global options may appear before or after a subcommand. A query requires either `--context-file` or
`--context`; when both are supplied, the file is used.

Interactive mode prompts once for a query and then for a context file or pasted context:

```sh
cargo run -p rslm-cli -- interactive
```

Generate Nushell completions:

```sh
cargo run -p rslm-cli -- completions > rslm-completions.nu
```

## Chunk Store And Retrieval

Ingest a document into SQLite using paragraph, fixed-window, or line chunking:

```sh
cargo run -p rslm-cli -- \
  --store context.db \
  ingest --context-file docs/demo/context.txt \
  --strategy paragraph \
  --doc-id mini-redis
```

Then query with store functions enabled:

```sh
cargo run -p rslm-cli -- \
  --store context.db \
  --doc-id mini-redis \
  query "How are expired keys removed?" \
  --context-file docs/demo/context.txt
```

Explicit `ingest` commands use an OpenAI embedder when `OPENAI_API_KEY` is present. Those chunks
receive semantic vectors, and `ctx_hybrid` combines semantic and BM25 rankings with RRF. Without
the key, explicit ingestion stores text only and hybrid calls fall back to BM25.

A store-backed query automatically ingests the supplied context with paragraph chunking when its
document ID has no chunks. This query-time ingestion does not use an embedder, even when
`OPENAI_API_KEY` is set. To populate semantic vectors, explicitly run `ingest` with
`OPENAI_API_KEY` before querying that document ID.

Chunking defaults and current fixed parameters are:

- `paragraph`: split on blank lines; this is the CLI default.
- `fixed`: 1,000-byte windows with 100 bytes of overlap.
- `line`: groups 50 lines per chunk.

An unrecognized `--strategy` currently falls back to paragraph chunking rather than returning an
error.

## Rust API

`rslm-core` accepts any `Arc<dyn LlmProvider>`:

```rust
use std::sync::Arc;

use rslm_core::Rlm;
use rslm_providers::{LlmProvider, OpenAiProvider};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider: Arc<dyn LlmProvider> = Arc::new(OpenAiProvider::new("gpt-4o"));
    let rlm = Rlm::new(provider, 5, 20, false);
    let answer = rlm
        .run("What database is used?", "The service stores data in SQLite.")
        .await?;
    println!("{answer}");
    Ok(())
}
```

Enable chunk-store functions when depending directly on `rslm-core`:

```toml
[dependencies]
rslm-core = { path = "../rslm/crates/rslm-core", features = ["store"] }
```

Implement `rslm_providers::LlmProvider` to supply another model backend. The deterministic
`rslm_harness::Harness` can drive the loop with scripted model responses in tests.

## Cargo Features

| Package | Feature | Effect |
| --- | --- | --- |
| `rslm-core` | `store` | Adds `ChunkStore` integration and store-backed Rhai functions |
| `rslm-providers` | `live-tests` | Enables tests that call real provider APIs |
| `rslm-store` | `live-tests` | Enables the real OpenAI embedding test |

The CLI enables `rslm-core/store` automatically.

## Development And Testing

```sh
cargo check --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Run provider live tests only with credentials and explicit opt-in:

```sh
cargo test -p rslm-providers --features live-tests -- live_tests
```

Criterion benches live in `rslm-bench`:

```sh
cargo bench -p rslm-bench
```

The deterministic harness directly exercises the six core Rhai registrations: `ctx_len`,
`ctx_slice`, `ctx_grep`, `print_cell`, `rlm_call`, and `final_answer`. It also covers recursive
delegation and its depth guard, iteration limits, script-error feedback, and provider message
recording without network calls. It does not exercise the store-backed `doc_id`, `ctx_chunks`,
`ctx_chunk`, `ctx_search`, or `ctx_hybrid` registrations. Store tests separately cover chunking,
idempotent ingestion, BM25, embeddings, HNSW, and RRF behavior.

## Documentation

- [Demo guide](docs/demo/README.md) and [demo script](docs/demo/demo.nu)
- [Human-readable reference](docs/assets/rslm.ref.md)
- [Original design](docs/superpowers/specs/2026-04-23-rslm-design.md)
- [Harness plan](docs/plans/2026-05-06-rslm-harness.md)
- [Generated architecture and feature pages](docs/assets/index.html)

Some design and generated reference documents describe intended behavior and may lag the source;
the current Rust implementation and tests are authoritative.

## License

Cargo workspace and package metadata declare dual licensing under MIT or Apache-2.0. The
repository currently has no root license files containing the MIT or Apache-2.0 license texts.
