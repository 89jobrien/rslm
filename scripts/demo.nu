#!/usr/bin/env nu
# rslm demo — three queries against the mini-redis source code.
# Source: https://github.com/tokio-rs/mini-redis (Apache-2.0 / MIT)
#
# Usage: nu scripts/demo.nu [--provider openai|anthropic] [--model <id>]
# Requires OPENAI_API_KEY or ANTHROPIC_API_KEY in the environment.

def rslm-query [query: string, ctx: string, provider: string, model: string] {
    let args = if ($model | is-empty) {
        ["--provider", $provider, "--verbose", "query", $query, "--context-file", $ctx]
    } else {
        ["--provider", $provider, "--model", $model, "--verbose", "query", $query, "--context-file", $ctx]
    }
    run-external "cargo" "run" "--bin" "rslm" "--" ...$args
}

def sep [label: string] {
    print $"\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    print $label
    print "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
}

def main [--provider: string = "openai", --model: string = ""] {
    let ctx = "docs/demo/context.txt"

    sep "Demo 1 — Keyword lookup (ctx_grep)\nQuery: How does mini-redis limit the number of concurrent connections?"
    rslm-query "How does mini-redis limit the number of concurrent connections?" $ctx $provider $model

    sep "Demo 2 — Multi-step slice and grep\nQuery: How does Db handle key expiration — data structures and background task?"
    rslm-query "How does the Db handle key expiration? What data structures are used and how does the background task learn about new expirations?" $ctx $provider $model

    sep "Demo 3 — Cross-module comparison (exercises rlm_call delegation)\nQuery: Compare server shutdown vs Db shutdown — signals and wait conditions"
    rslm-query "Compare how the server handles graceful shutdown versus how the Db handles shutdown. What signals does each use and what do they wait for?" $ctx $provider $model

    print "\nDemo complete."
}
