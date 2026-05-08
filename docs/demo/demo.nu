#!/usr/bin/env nu
# rslm demo — three queries against the mini-redis source code.
# Source: https://github.com/tokio-rs/mini-redis (Apache-2.0 / MIT)
#
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
    print "Query: How does mini-redis limit the number of concurrent connections?"
    print "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
    run-external "cargo" "run" "--bin" "rslm" "--" ...$base_args
        "query" "How does mini-redis limit the number of concurrent connections?"
        "--context-file" $ctx_file

    print "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    print "Demo 2 — Multi-step slice and grep"
    print "Query: How does the Db handle key expiration? What data structures are used"
    print "       and how does the background task learn about new expirations?"
    print "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
    run-external "cargo" "run" "--bin" "rslm" "--" ...$base_args
        "query" "How does the Db handle key expiration? What data structures are used and how does the background task learn about new expirations?"
        "--context-file" $ctx_file

    print "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    print "Demo 3 — Cross-module comparison (exercises rlm_call delegation)"
    print "Query: Compare how the server handles graceful shutdown versus how the Db"
    print "       handles shutdown. What signals does each use and what do they wait for?"
    print "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
    run-external "cargo" "run" "--bin" "rslm" "--" ...$base_args
        "query" "Compare how the server handles graceful shutdown versus how the Db handles shutdown. What signals does each use and what do they wait for?"
        "--context-file" $ctx_file

    print "\nDemo complete."
}
