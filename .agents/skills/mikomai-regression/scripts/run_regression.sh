#!/usr/bin/env bash
set -u

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$repo_root" || exit 2

passed=0
failed=0

run_case() {
  local name="$1"
  shift
  printf '\n[%s]\n' "$name"
  if "$@"; then
    printf 'PASS: %s\n' "$name"
    passed=$((passed + 1))
  else
    printf 'FAIL: %s\n' "$name"
    failed=$((failed + 1))
  fi
}

run_case "Rust workspace suite" cargo test --workspace

run_case "Greeting and Ping/Traceroute presentation" bash -c '
  ./mikomai-desktop-mac/test-core.sh --filter GreetingPresentationTests &&
  ./mikomai-desktop-mac/test-core.sh --filter ExecutionTerminalPresentationTests
'

run_case "Native probe results and greeting rendering" ./mikomai-desktop-mac/test-execution-queue.sh

run_case "Current F220 RAG-backed CLI answer" bash -c '
  output="$(npm run --silent cli -- chat "F220のVLAN設定方法を教えて")" || exit 1
  printf "%s\n" "$output" | rg -q "Fitelnet"
  printf "%s\n" "$output" | rg -q "vlan-id"
  printf "%s\n" "$output" | rg -q "02-2_make_access_vlan.md"
'

printf '\nRegression summary: %d passed, %d failed\n' "$passed" "$failed"
if [ "$failed" -ne 0 ]; then
  exit 1
fi
