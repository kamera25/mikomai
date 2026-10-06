#!/bin/sh
set -eu
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$ROOT"
cargo build -p mikomai-bindings
OUTPUT="$(mktemp -d /tmp/mikomai-generated.XXXXXX)"
trap 'rm -rf "$OUTPUT"' EXIT
cargo run -p mikomai-bindings --bin uniffi-bindgen -- generate --library target/debug/libmikomai_bindings.dylib --language swift --config crates/mikomai-bindings/uniffi.toml --out-dir "$OUTPUT"
cp "$OUTPUT/MikomaiBindings.swift" mikomai-desktop-mac/Sources/MikomaiBindings/
cp "$OUTPUT/MikomaiGeneratedFFI.h" mikomai-desktop-mac/Sources/MikomaiGeneratedFFI/include/
cp "$OUTPUT/MikomaiGeneratedFFI.modulemap" mikomai-desktop-mac/Sources/MikomaiGeneratedFFI/include/module.modulemap
"${MIKOMAI_CSHARP_BINDGEN:-uniffi-bindgen-cs}" --library target/debug/libmikomai_bindings.dylib --config crates/mikomai-bindings/uniffi.toml --out-dir contracts/csharp/Generated

# Keep generated output reproducible and free of generator whitespace noise.
python3 - <<'NORMALIZE'
from pathlib import Path
paths = [Path('mikomai-desktop-mac/Sources/MikomaiBindings/MikomaiBindings.swift'), Path('mikomai-desktop-mac/Sources/MikomaiGeneratedFFI/include/MikomaiGeneratedFFI.h')]
paths += list(Path('contracts/csharp/Generated').glob('*.cs'))
for path in paths:
    path.write_text('\n'.join(line.rstrip() for line in path.read_text().splitlines()) + '\n')
NORMALIZE
