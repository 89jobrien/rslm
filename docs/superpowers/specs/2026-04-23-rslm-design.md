# rslm — Recursive Language Model (Rust Implementation)

**Date:** 2026-04-23
**Based on:** [Recursive Language Models](https://alexzhang13.github.io/blog/2025/rlm/) — Zhang & Khattab, 2025

---

## Overview

`rslm` is a Rust implementation of the Recursive Language Model (RLM) inference strategy. An RLM
wraps an LLM call such that the model never directly sees the full context. Instead, the context
is stored in a Rhai scripting environment and the model interacts with it via registered functions
— slicing, grepping, and spawning recursive sub-calls — before emitting a final answer.

The result is effectively unbounded context handling with no architecture changes to the underlying
model, at comparable or lower cost than feeding the full context directly.

---

## Goals

- Implement the core RLM loop: model writes Rhai scripts, we execute them, results feed back in
- Support recursive `rlm_call` dispatch to child RLM instances with isolated environments
- Trait-based LLM provider abstraction (OpenAI default, Anthropic available)
- CLI with file-based, inline, and interactive modes
- Verbose mode streams the recursive trace (depth indicators + cell outputs) to terminal

## Non-Goals

- Python REPL (the paper uses Python; we use Rhai — same conceptual role, no subprocess)
- Training or fine-tuning
- Async parallel recursive calls (blocked calls only for now; async is future work)
- Prefix caching or cost control guarantees

---

## Architecture

3-crate Cargo workspace:

```
rslm/
  Cargo.toml                        # workspace
  crates/
    rslm-core/                      # RLM engine + Rhai environment
      src/
        lib.rs
        rlm.rs                      # Rlm struct, recursive dispatch loop
        env.rs                      # Rhai engine setup, context variable, registered fns
        protocol.rs                 # StepResult enum, notebook types
    rslm-providers/                 # LlmProvider trait + impls
      src/
        lib.rs
        provider.rs                 # trait + Message/Role types
        openai.rs                   # async-openai, OPENAI_API_KEY
        anthropic.rs                # reqwest, ANTHROPIC_API_KEY
    rslm-cli/                       # binary
      src/
        main.rs
        interactive.rs
        file_mode.rs
```

---

## Core Data Flow

1. `Rlm::run(query, context)` is called. Context stored in Rhai engine as `let ctx = "...";`.
2. System prompt explains available Rhai functions (see Environment section).
3. Model generates a Rhai script. Engine executes it.
4. If the script calls `rlm_call(q, c)` — intercepted as a registered Rhai function. A child
   `Rlm` (depth+1) is constructed with a new isolated engine, run to completion via
   `tokio::task::spawn_blocking`, and its answer returned as the Rhai call's return value.
5. If the script calls `final(answer)` — a flag is set on the engine; the loop detects it and
   exits, bubbling the answer up.
6. Otherwise — script output appended to the notebook as a `Cell { script, output }`. Loop
   continues; next model call receives system prompt + query + all previous cells as message
   history.
7. Depth limit (default: 5) hard-stops infinite recursion with an error.

---

## Key Types

```rust
// rslm-core

pub struct Rlm {
    provider: Arc<dyn LlmProvider>,
    depth: usize,
    max_depth: usize,
}

pub struct Notebook {
    cells: Vec<Cell>,
}

pub struct Cell {
    pub script: String,
    pub output: String,
}

pub enum StepResult {
    Continue(String),   // script output, loop again
    Final(String),      // answer, exit loop
}
```

---

## Rhai Environment

The Rhai engine exposes these functions to the model. The raw context is never sent to the LLM —
only what the model explicitly extracts via these calls appears in message history.

| Function     | Signature                                | Description                                         |
| ------------ | ---------------------------------------- | --------------------------------------------------- |
| `ctx_slice`  | `(start: int, end: int) -> String`       | Byte-range slice of context                         |
| `ctx_grep`   | `(pattern: String) -> String`            | Regex search; returns matching lines joined by `\n` |
| `ctx_len`    | `() -> int`                              | Total context byte length                           |
| `rlm_call`   | `(query: String, ctx: String) -> String` | Spawn child RLM; blocking                           |
| `print_cell` | `(msg: String)`                          | Append message to cell output log                   |
| `final`      | `(answer: String)`                       | Signal loop exit with this answer                   |

`rlm_call` bridges async/sync via `tokio::task::spawn_blocking`. Each child gets its own
isolated Rhai engine with `ctx` set to the passed fragment.

---

## LLM Provider Trait

```rust
// rslm-providers

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, messages: Vec<Message>) -> Result<String>;
    fn model_id(&self) -> &str;
}

pub struct Message { pub role: Role, pub content: String }
pub enum Role { System, User, Assistant }
```

Provider and model selected via environment variables:

| Variable        | Default  | Options                                  |
| --------------- | -------- | ---------------------------------------- |
| `RSLM_PROVIDER` | `openai` | `openai`, `anthropic`                    |
| `RSLM_MODEL`    | `gpt-4o` | any model string for the chosen provider |

`OpenAiProvider` — `async-openai` crate, reads `OPENAI_API_KEY`.
`AnthropicProvider` — `reqwest` against Messages API, reads `ANTHROPIC_API_KEY`, default
`claude-sonnet-4-6`.

---

## CLI

```
rslm query "<query>" --context-file <path>     # file-based context
rslm query "<query>" --context "<string>"      # inline context
rslm interactive                               # interactive mode (query + context from stdin)

Flags:
  --max-depth <n>              default: 5
  --verbose                    print Rhai cells + outputs as they execute, with [depth=N] prefix
  --provider openai|anthropic  override RSLM_PROVIDER
  --model <model-id>           override RSLM_MODEL
```

Interactive mode prompts for query, reads context from stdin or a file path, then streams the
recursive trace to terminal. Each cell execution prints:

```
[depth=0] >> <script>
[depth=0] << <output>
[depth=1] rlm_call: "<sub-query>" (ctx: 2048 bytes)
...
[depth=0] FINAL: <answer>
```

---

## Error Handling

- Depth limit exceeded → `RlmError::MaxDepthExceeded`
- Rhai script compile/runtime error → `RlmError::ScriptError(msg)` — fed back to model as cell
  output so it can self-correct (up to 3 retries per cell, then hard error)
- LLM provider error → `RlmError::ProviderError(anyhow::Error)` — non-retryable, propagates up
- `final()` not called after max iterations (default: 20) → `RlmError::MaxIterationsExceeded`

---

## Testing Strategy

- `rslm-core`: unit tests for Rhai env functions (mock context strings), notebook accumulation,
  depth limit enforcement, `StepResult` parsing
- `rslm-providers`: integration tests behind a feature flag `live-tests`; default tests use a
  mock `LlmProvider` that returns canned scripts
- `rslm-cli`: CLI integration tests with mock provider injected via env var or test helper

---

## Dependencies

| Crate                            | Purpose                    |
| -------------------------------- | -------------------------- |
| `rhai`                           | Embedded scripting engine  |
| `async-openai`                   | OpenAI provider            |
| `reqwest`                        | Anthropic provider HTTP    |
| `tokio`                          | Async runtime              |
| `async-trait`                    | Trait object async methods |
| `anyhow`                         | Error handling             |
| `regex`                          | `ctx_grep` implementation  |
| `clap`                           | CLI argument parsing       |
| `tracing` / `tracing-subscriber` | Structured logging         |
