# rslm — Agent Operating Guide

Rust implementation of Recursive Language Model (RLM) inference. Context is never
sent to LLM — model writes Rhai scripts that execute in an isolated engine. Each
script runs with fresh Scope state; no persistence between cells by design.

## Build, Lint, and Test Commands

### Quick Reference

```bash
# Build all crates
cargo build --workspace

# Run all tests (rslm-providers requires live-tests flag for real API calls)
cargo test --workspace

# Lint all code
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt --all
```

### Running Individual Tests

```bash
# Test specific crate
cd crates/rslm-core && cargo test
cd crates/rslm-providers && cargo test --features live-tests

# Run a specific test function
cargo test test_function_name

# Run tests matching a pattern
cargo test pattern_to_match

# Use cargo-nextest for faster parallel runs
cargo nextest run --workspace
```

### Known Gotchas

- `rlm_call` uses `tokio::runtime::Builder::new_current_thread()` to bridge
  async→sync. This panics inside tokio test runtime. Tests exercising `rlm_call`
  must be `#[ignore]`d with a note, or restructured to use `spawn_blocking`.
- `strip_code_fences` and other private helpers need `pub(crate)` to be
  reachable from `src/tests.rs`.
- `thiserror` is not auto-included — add it explicitly to `[workspace.dependencies]`
  and each crate.

## Workspace Layout

```
rslm/
├── crates/
│   ├── rslm-core/      # RLM loop + Rhai environment
│   ├── rslm-providers/ # Provider trait + OpenAI/Anthropic impls
│   ├── rslm-cli/       # Binary entrypoint
│   ├── rslm-harness/   # Test harness + provider testing
│   ├── rslm-store/     # Session/cell storage (SQLite backend)
│   └── rslm-bench/     # Criterion benchmarks
├── docs/
│   └── superpowers/specs/2026-04-23-rslm-design.md
└── Cargo.toml          # Workspace configuration
```

## Provider Configuration

### Environment Variables

```bash
# Provider and model selection (runtime)
export RSLM_PROVIDER=openai          # or: anthropic
export RSLM_MODEL=gpt-4o             # Default: gpt-4o

# API keys
export OPENAI_API_KEY=sk-...         # For openai provider
export ANTHROPIC_API_KEY=sk-ant-...  # For anthropic provider
```

### Defaults

- **Provider**: openai
- **OpenAI model**: gpt-4o
- **Anthropic model**: claude-sonnet-4-6

## Code Style Guidelines

### Rust Version & Toolchain

- **Edition**: 2021
- **Components**: rustfmt, clippy
- **Max width**: 100 characters

### Naming Conventions

- **Structs/Enums**: PascalCase (`RlmState`, `CellOutput`)
- **Functions/Methods**: snake_case (`execute_cell`, `get_session`)
- **Constants**: SCREAMING_SNAKE_CASE
- **Modules**: snake_case (`provider`, `executor`)
- **Files**: snake_case.rs (`executor.rs`)

### Error Handling

- **Primary**: `anyhow::Result<T>` for fallible operations
- **Avoid**: `unwrap()` and `expect()` in production code
- **Pattern**: Propagate errors up with `?` operator

### Module Organization

- **Core logic**: `rslm-core/src/lib.rs` (RLM loop)
- **Rhai engine**: `rslm-core/src/engine.rs` (Scope, script execution)
- **Providers**: `rslm-providers/src/provider.rs` (trait), submodules per impl
- **CLI**: `rslm-cli/src/main.rs` (clap, file/interactive modes)
- **Tests**: Inline in implementation files using `#[cfg(test)] mod tests`

### Key Dependencies

- **Rhai**: `1.x` (scripting engine; requires `features = ["sync"]` for Send+Sync)
- **async-openai**: `0.26` (OpenAI client)
- **reqwest**: `0.12` with JSON feature (HTTP)
- **tokio**: `1.x` with full features
- **serde/serde_json**: Serialization
- **anyhow/thiserror**: Error handling
- **clap**: CLI with derive feature
- **tracing**: Logging

## Architecture

RLM runs an iterative loop: model receives Rhai code to execute (with context),
engine runs it in a fresh Scope, yields output back to model. Context never goes
to LLM directly — only Rhai execution results are fed back.

### Protocol

See `/Users/joe/dev/rslm/crates/rslm-core/src/protocol.rs` for LLM request/response
shapes. Each turn:

1. **Model writes**: Rhai script (in code block)
2. **Engine executes**: Script in isolated Scope
3. **Output captured**: Serialized back to model
4. **Loop continues**: Until model reaches terminal state

## Development Workflow

1. **Start**: `cargo check --workspace` to validate all crates
2. **Code**: Edit in your crate, run tests frequently
3. **Test**: `cargo test --workspace` before commit
4. **Lint**: `cargo clippy --all-targets -- -D warnings`
5. **Format**: `cargo fmt --all` to auto-format

## Commit Guidelines

- Format: `<type>(<scope>): <description>`
- Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`
- Scope: crate name (e.g., `rslm-core`, `rslm-providers`)
- Example: `feat(rslm-core): add Rhai error context to script results`
