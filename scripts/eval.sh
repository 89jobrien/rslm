#!/usr/bin/env sh
# Live accuracy eval — runs each golden query against a real LLM and scores the answer.
#
# Usage: ./scripts/eval.sh [--provider openai|anthropic] [--model <id>] [--out <path>]
# Requires OPENAI_API_KEY or ANTHROPIC_API_KEY.
# Output: JSON report written to --out (default: /tmp/rslm-eval-report.json)

set -e

PROVIDER="openai"
MODEL=""
CTX="docs/demo/context.txt"
GOLDEN="crates/rslm-bench/golden/queries.json"
OUT="/tmp/rslm-eval-report.json"

while [ $# -gt 0 ]; do
    case "$1" in
        --provider) PROVIDER="$2"; shift 2 ;;
        --model)    MODEL="$2";    shift 2 ;;
        --out)      OUT="$2";      shift 2 ;;
        *) echo "Unknown arg: $1"; exit 1 ;;
    esac
done

if [ -n "$MODEL" ]; then
    BASE="--provider $PROVIDER --model $MODEL"
else
    BASE="--provider $PROVIDER"
fi

total=0
passed=0
results='[]'

# Read each case from golden JSON
case_count=$(jq 'length' "$GOLDEN")
i=0
while [ "$i" -lt "$case_count" ]; do
    id=$(jq -r ".[$i].id" "$GOLDEN")
    query=$(jq -r ".[$i].query" "$GOLDEN")
    patterns=$(jq -c ".[$i].expected_patterns" "$GOLDEN")

    printf "\n[%d/%d] %s\n" "$((i+1))" "$case_count" "$id"
    printf "  Query: %s\n" "$query"

    start_ms=$(date +%s%3N 2>/dev/null || date +%s)
    # shellcheck disable=SC2086
    answer=$(cargo run --bin rslm -- $BASE query "$query" --context-file "$CTX" 2>/dev/null)
    end_ms=$(date +%s%3N 2>/dev/null || date +%s)
    latency=$((end_ms - start_ms))

    # Score: check each pattern (case-insensitive)
    case_passed=true
    failed_patterns='[]'
    pattern_count=$(echo "$patterns" | jq 'length')
    j=0
    while [ "$j" -lt "$pattern_count" ]; do
        pattern=$(echo "$patterns" | jq -r ".[$j]")
        lower_answer=$(echo "$answer" | tr '[:upper:]' '[:lower:]')
        lower_pattern=$(echo "$pattern" | tr '[:upper:]' '[:lower:]')
        if ! echo "$lower_answer" | grep -qF "$lower_pattern"; then
            case_passed=false
            failed_patterns=$(echo "$failed_patterns" | jq --arg p "$pattern" '. + [$p]')
        fi
        j=$((j+1))
    done

    if $case_passed; then
        printf "  PASS (%dms)\n" "$latency"
        passed=$((passed+1))
    else
        printf "  FAIL (%dms) — missing: %s\n" "$latency" "$(echo "$failed_patterns" | jq -r 'join(", ")')"
        printf "  Answer: %s\n" "$answer"
    fi

    result=$(jq -n \
        --arg id "$id" \
        --arg query "$query" \
        --arg answer "$answer" \
        --argjson passed "$case_passed" \
        --argjson failed "$failed_patterns" \
        --argjson latency "$latency" \
        '{id:$id, query:$query, answer:$answer, passed:$passed, failed_patterns:$failed, latency_ms:$latency}')
    results=$(echo "$results" | jq --argjson r "$result" '. + [$r]')

    total=$((total+1))
    i=$((i+1))
done

pass_rate=$(echo "scale=4; $passed / $total" | bc)
report=$(jq -n \
    --argjson total "$total" \
    --argjson passed "$passed" \
    --argjson failed "$((total-passed))" \
    --argjson rate "$pass_rate" \
    --argjson results "$results" \
    '{total:$total, passed:$passed, failed:$failed, pass_rate:$rate, results:$results}')

echo "$report" > "$OUT"

printf "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
printf "Results: %d/%d passed (%.0f%%)\n" "$passed" "$total" "$(echo "$pass_rate * 100" | bc)"
printf "Report:  %s\n" "$OUT"
