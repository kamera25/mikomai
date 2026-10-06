#!/bin/sh
set -eu
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
"${MIKOMAI_BUILD_PYTHON:-$ROOT/venv/bin/python}" -m PyInstaller --noconfirm --clean --distpath "$ROOT/target/debug" --workpath "$ROOT/target/device-worker-build" "$ROOT/workers/device-worker/worker.spec"
