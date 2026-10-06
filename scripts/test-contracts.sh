#!/bin/sh
set -eu
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$ROOT"
RESULTS="${MIKOMAI_CONTRACT_RESULTS_DIR:-$(mktemp -d /tmp/mikomai-test-contracts.XXXXXX)}"
mkdir -p "$RESULTS"
export DYLD_LIBRARY_PATH="$ROOT/target/debug${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
cargo build -p mikomai-bindings -p mikomai-cli
MIKOMAI_GRAPH_DB_PATH="$RESULTS/cli-db" target/debug/mikomai-cli contract contracts/fixtures/task-lifecycle.json > "$RESULTS/cli.json"
MIKOMAI_GRAPH_DB_PATH="$RESULTS/csharp-db" "${MIKOMAI_DOTNET:-dotnet}" run --project contracts/csharp/Contracts.csproj -- contracts/fixtures/task-lifecycle.json > "$RESULTS/csharp.json"
MIKOMAI_GRAPH_DB_PATH="$RESULTS/swift-db" MIKOMAI_CONTRACT_RESULT_PATH="$RESULTS/swift.json" sh mikomai-desktop-mac/test-core.sh --filter TaskContractTests > "$RESULTS/swift.log" 2>&1
python3 - "$RESULTS" <<'PY'
import json,sys
from pathlib import Path
root=Path(sys.argv[1]);expected=json.loads(Path('contracts/fixtures/task-lifecycle.expected.json').read_text())
for language in ('cli','csharp','swift'):
    actual=json.loads((root/(language+'.json')).read_text())
    assert actual==expected,(language,actual)
print('PASS Swift / C# / CLI: matching ordered TaskEvents, Japanese result, callback/query recovery')
PY
printf 'Contract results: %s\n' "$RESULTS"
