#!/usr/bin/env nu
# rslm demo — three queries that exercise ctx_grep, ctx_slice, and rlm_call (via multi-step).
# Requires OPENAI_API_KEY or ANTHROPIC_API_KEY in the environment.
# Usage: nu docs/demo/demo.nu [--provider openai|anthropic] [--model <id>]

def main [
    --provider: string = "openai"  # openai or anthropic
    --model: string = ""           # override model (default: provider default)
] {
    let ctx_file = "docs/demo/context.txt"
    let base_args = if ($model | is-empty) {
        ["--provider", $provider, "--verbose"]
    } else {
        ["--provider", $provider, "--model", $model, "--verbose"]
    }

    print "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    print "Demo 1 — Keyword lookup (ctx_grep)"
    print "Query: What are the three ownership rules in Rust?"
    print "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
    run-external "cargo" "run" "--bin" "rslm" "--" ...$base_args
        "query" "What are the three ownership rules in Rust?"
        "--context-file" $ctx_file

    print "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    print "Demo 2 — Multi-step slice and grep"
    print "Query: How does Rust handle concurrency safety, and what are Send and Sync?"
    print "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
    run-external "cargo" "run" "--bin" "rslm" "--" ...$base_args
        "query" "How does Rust handle concurrency safety, and what are Send and Sync?"
        "--context-file" $ctx_file

    print "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    print "Demo 3 — Cross-section comparison (tests rlm_call delegation)"
    print "Query: Compare Rust's approach to error handling with its approach to concurrency."
    print "        Which section covers each topic, and what do they have in common?"
    print "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
    run-external "cargo" "run" "--bin" "rslm" "--" ...$base_args
        "query" "Compare Rust's approach to error handling with its approach to concurrency. Which section covers each topic, and what do they have in common?"
        "--context-file" $ctx_file

    print "\nDemo complete."
}
