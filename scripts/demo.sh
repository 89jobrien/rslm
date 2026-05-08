#!/usr/bin/env sh
# rslm demo — three queries against the mini-redis source code.
# Source: https://github.com/tokio-rs/mini-redis (Apache-2.0 / MIT)
#
# Usage: ./scripts/demo.sh [--provider openai|anthropic] [--model <id>]
# Requires OPENAI_API_KEY or ANTHROPIC_API_KEY in the environment.

set -e

PROVIDER="openai"
MODEL=""
CTX="docs/demo/context.txt"

while [ $# -gt 0 ]; do
    case "$1" in
        --provider) PROVIDER="$2"; shift 2 ;;
        --model)    MODEL="$2";    shift 2 ;;
        *) echo "Unknown arg: $1"; exit 1 ;;
    esac
done

if [ -n "$MODEL" ]; then
    BASE="--provider $PROVIDER --model $MODEL --verbose"
else
    BASE="--provider $PROVIDER --verbose"
fi

rslm() {
    # shellcheck disable=SC2086
    cargo run --bin rslm -- $BASE query "$1" --context-file "$CTX"
}

echo
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Demo 1 — Keyword lookup (ctx_grep)"
echo "Query: How does mini-redis limit the number of concurrent connections?"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo
rslm "How does mini-redis limit the number of concurrent connections?"

echo
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Demo 2 — Multi-step slice and grep"
echo "Query: How does Db handle key expiration — data structures and background task?"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo
rslm "How does the Db handle key expiration? What data structures are used and how does the background task learn about new expirations?"

echo
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Demo 3 — Cross-module comparison (exercises rlm_call delegation)"
echo "Query: Compare server shutdown vs Db shutdown — signals and wait conditions"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo
rslm "Compare how the server handles graceful shutdown versus how the Db handles shutdown. What signals does each use and what do they wait for?"

echo
echo "Demo complete."
