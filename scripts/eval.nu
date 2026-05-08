#!/usr/bin/env nu
# Live accuracy eval — runs each golden query against a real LLM and scores the answer.
#
# Usage: nu scripts/eval.nu [--provider openai|anthropic] [--model <id>] [--out <path>]
# Requires OPENAI_API_KEY or ANTHROPIC_API_KEY.
# Output: JSON report written to --out (default: /tmp/rslm-eval-report.json)

def score-answer [answer: string, patterns: list<string>] {
    let lower = ($answer | str downcase)
    let failed = ($patterns | where { |p| not ($lower | str contains ($p | str downcase)) })
    { passed: ($failed | is-empty), failed_patterns: $failed }
}

def run-query [query: string, ctx: string, provider: string, model: string] {
    let base = if ($model | is-empty) {
        ["--provider", $provider]
    } else {
        ["--provider", $provider, "--model", $model]
    }
    let args = ($base | append ["query", $query, "--context-file", $ctx])
    do { run-external "cargo" "run" "--bin" "rslm" "--" ...$args } | complete | get stdout | str trim
}

def main [
    --provider: string = "openai"
    --model: string = ""
    --out: string = "/tmp/rslm-eval-report.json"
] {
    let ctx = "docs/demo/context.txt"
    let golden = (open crates/rslm-bench/golden/queries.json)
    let total = ($golden | length)

    print $"Running ($total) eval cases against provider=($provider)\n"

    let results = ($golden | enumerate | each { |item|
        let case = $item.item
        let i = $item.index
        print $"[($i + 1)/($total)] ($case.id)"
        print $"  Query: ($case.query)"

        let start = (date now | into int)
        let answer = (run-query $case.query $ctx $provider $model)
        let latency_ms = (((date now | into int) - $start) / 1_000_000)

        let scored = (score-answer $answer $case.expected_patterns)

        if $scored.passed {
            print $"  PASS \((($latency_ms) | into string)ms\)"
        } else {
            print $"  FAIL \((($latency_ms) | into string)ms\) — missing: ($scored.failed_patterns | str join ', ')"
            print $"  Answer: ($answer)"
        }

        {
            id: $case.id
            query: $case.query
            answer: $answer
            passed: $scored.passed
            failed_patterns: $scored.failed_patterns
            latency_ms: $latency_ms
        }
    })

    let passed = ($results | where passed | length)
    let failed = ($total - $passed)
    let pass_rate = (if $total > 0 { $passed / $total } else { 0.0 })

    let report = {
        total: $total
        passed: $passed
        failed: $failed
        pass_rate: $pass_rate
        results: $results
    }

    $report | to json | save --force $out

    print $"\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    print $"Results: ($passed)/($total) passed \(($pass_rate * 100 | math round)%\)"
    print $"Report:  ($out)"
}
