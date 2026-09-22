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
run_case "Frontend unit suite" npm test -- --run

run_case "P0 repeated Graph query guard" \
  cargo test --manifest-path src-tauri/Cargo.toml \
    stops_consecutive_identical_graph_queries --lib
run_case "P1 one digit MAC octet" \
  cargo test --manifest-path src-tauri/Cargo.toml \
    short_mac_octet_uses_endpoint_lookup_and_then_ping --lib
run_case "P1 Cisco MAC notation" \
  cargo test --manifest-path src-tauri/Cargo.toml \
    cisco_mac_format_uses_the_same_endpoint_lookup --lib
run_case "P1 standard colon MAC lookup" \
  cargo test --manifest-path src-tauri/Cargo.toml \
    mac_goal_uses_structured_lookup_instead_of_text_query --lib
run_case "P1 resolved MAC is pinged" \
  cargo test --manifest-path src-tauri/Cargo.toml \
    host_reachability_goal_constrains_query_and_pings_resolved_ip --lib
run_case "P2 empty Graph recovery" \
  cargo test --manifest-path src-tauri/Cargo.toml \
    empty_mac_graph_result_fetches_registered_arp_before_asking_human --lib
run_case "P2 failed ARP is not absence" \
  cargo test --manifest-path src-tauri/Cargo.toml \
    failed_local_arp_observation_is_not_reported_as_absent --lib
run_case "P2 localhost ARP lookup" \
  cargo test --manifest-path src-tauri/Cargo.toml \
    localhost_arp_mac_lookup_uses_local_state_and_finishes_from_its_entries --lib
run_case "P2 MAC planner schema" \
  cargo test --manifest-path src-tauri/Cargo.toml \
    short_mac_reachability_schema_excludes_free_text_graph_queries --lib

run_case "P3 current CLI baseline" bash -c '
  output="$(npm run --silent cli -- chat "F220のVLAN設定方法を教えて")" || exit 1
  printf "%s\n" "$output" | rg -q "Fitelnetでの Trunk VLAN の追加方法"
  printf "%s\n" "$output" | rg -q "Fitelnetでの Access VLAN の追加方法"
  printf "%s\n" "$output" | rg -q "show mac address-table"
'

printf '\nRegression summary: %d passed, %d failed\n' "$passed" "$failed"
if [ "$failed" -ne 0 ]; then
  exit 1
fi
