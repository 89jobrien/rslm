# rslm demo

Demonstrates the three core RLM capabilities against a Rust programming language reference doc:

| Demo | Capability | Query |
|------|-----------|-------|
| 1 | `ctx_grep` — keyword lookup | "What are the three ownership rules in Rust?" |
| 2 | `ctx_slice` + multi-step | "How does Rust handle concurrency safety, and what are Send and Sync?" |
| 3 | `rlm_call` — recursive delegation | "Compare error handling with concurrency — which section covers each?" |

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
  "What are the borrowing rules?" \
  --context-file docs/demo/context.txt
```

## What to look for

With `--verbose`, the terminal prints each Rhai cell as it executes:

```
[depth=0] >> ctx_grep("ownership")
[depth=0] << Each value in Rust has an owner...
[depth=0] >> final_answer("1. Each value...")
[depth=0] FINAL: 1. Each value...
```

For Demo 3, watch for `[depth=1]` lines — those are child RLM instances spawned by `rlm_call`
to answer sub-questions against focused context slices.

## Context file

`context.txt` is a structured reference covering seven Rust topics: Ownership, Borrowing,
Lifetimes, Traits, Concurrency, Error Handling, and Performance. Each section is clearly
delimited with `== Section N: Title ==` headings that the model can grep for.
