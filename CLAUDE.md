# rslm

Rust implementation of Recursive Language Model (RLM) inference. See `docs/superpowers/specs/2026-04-23-rslm-design.md`.

## Build & Test

- `cargo check --workspace` — validate all crates
- `cargo test --workspace` — run all tests (providers crate has no tests by default; add `live-tests`
  feature flag for real API calls)
- `rhai` crate requires `features = ["sync"]` in rslm-core — needed for `Send + Sync` engine closures

## Known Gotchas

- `rlm_call` uses `tokio::runtime::Builder::new_current_thread().block_on(...)` to bridge async→sync.
  This panics inside a tokio test runtime. Tests exercising `rlm_call` end-to-end must be
  `#[ignore]`d with a note, or restructured to use `spawn_blocking`.
- `strip_code_fences` and other private helpers need `pub(crate)` to be reachable from `src/tests.rs`.
- `thiserror` is not auto-included — add it explicitly to `[workspace.dependencies]` and each crate.

## Provider Selection

- `RSLM_PROVIDER=openai|anthropic` + `RSLM_MODEL=<id>` — env vars for runtime provider/model selection
- Default: openai / gpt-4o. Anthropic default: claude-sonnet-4-6
- Anthropic provider reads `ANTHROPIC_API_KEY`; OpenAI reads `OPENAI_API_KEY`

## Architecture

- 3-crate workspace: `rslm-core` (RLM loop + Rhai env), `rslm-providers` (trait + impls),
  `rslm-cli` (binary)
- Context never sent to LLM directly — model writes Rhai scripts, engine executes them
- Each script runs in a fresh `Scope::new()` — no state persists between cells (by design)
