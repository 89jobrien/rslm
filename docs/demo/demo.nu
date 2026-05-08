#!/usr/bin/env nu
# rslm demo — three queries against the mini-redis source code.
# Source: https://github.com/tokio-rs/mini-redis (Apache-2.0 / MIT)
#
# Requires OPENAI_API_KEY or ANTHROPIC_API_KEY in the environment.
# Usage: nu docs/demo/demo.nu [--provider openai|anthropic] [--model <id>]

def run-query [query: string, ctx_file: string, base_args: list<string>] {
    let args = ($base_args | append ["query", $query, "--context-file", $ctx_file])
    run-external "cargo" "run" "--bin" "rslm" "--" ...$args
}

def main [
    --provider: string = "openai"
    --model: string = ""
] {
    let ctx_file = "docs/demo/context.txt"
    let base_args = if ($model | is-empty) {
        ["--provider", $provider, "--verbose"]
    } else {
        ["--provider", $provider, "--model", $model, "--verbose"]
    }

    print "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    print "Demo 1 — Keyword lookup (ctx_grep)"
    print "Query: How does mini-redis limit the number of concurrent connections?"
    print "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
    run-query "How does mini-redis limit the number of concurrent connections?" $ctx_file $base_args

    print "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    print "Demo 2 — Multi-step slice and grep"
    print "Query: How does Db handle key expiration — data structures and background task?"
    print "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
    run-query "How does the Db handle key expiration? What data structures are used and how does the background task learn about new expirations?" $ctx_file $base_args

    print "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    print "Demo 3 — Cross-module comparison (exercises rlm_call delegation)"
    print "Query: Compare server shutdown vs Db shutdown — signals and wait conditions"
    print "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
    run-query "Compare how the server handles graceful shutdown versus how the Db handles shutdown. What signals does each use and what do they wait for?" $ctx_file $base_args

    print "\nDemo complete."
}
