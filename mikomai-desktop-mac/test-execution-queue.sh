#!/bin/sh
set -eu
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
APP="$ROOT/mikomai-desktop-mac"
SCRATCH="${MIKOMAI_SWIFT_SCRATCH_PATH:-/private/tmp/mikomai-swift-app-build}"
SDK="${MIKOMAI_MACOS_SDK:-/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk}"
CHECK_DIR="$(mktemp -d /private/tmp/mikomai-execution-check.XXXXXX)"
export MIKOMAI_EXECUTION_CHECK_DIR="$CHECK_DIR"
# Build the real application sources. The harness uses isolated preferences,
# temporary settings/audit/graph paths, no model, and only 127.0.0.1 probes.
sed '/^@main$/d' "$APP/Sources/MikomaiDesktopMac/MikomaiDesktopMac.swift" > "$CHECK_DIR/full.swift"
cat "${MIKOMAI_EXECUTION_CHECK_SOURCE:-$APP/Tests/ExecutionQueueChecks/ExecutionQueueChecks.swift}" >> "$CHECK_DIR/full.swift"
set -- "$CHECK_DIR/full.swift"
for source in "$APP"/Sources/MikomaiDesktopMac/*.swift; do
    case "$source" in */MikomaiDesktopMac.swift) ;; *) set -- "$@" "$source" ;; esac
done
swiftc -suppress-warnings -sdk "$SDK" -module-cache-path "$CHECK_DIR/cache" \
    -I "$SCRATCH/out/Products/Debug" \
    -Xcc "-fmodule-map-file=$SCRATCH/out/Intermediates.noindex/GeneratedModuleMaps/MikomaiGeneratedFFI.modulemap" \
    -L "$ROOT/target/debug" -lmikomai_bindings -Xlinker -rpath -Xlinker "$ROOT/target/debug" \
    "$SCRATCH/out/Products/Debug/MikomaiDesktopCore.o" "$SCRATCH/out/Products/Debug/MikomaiBindings.o" "$@" \
    "$SCRATCH/out/Intermediates.noindex/MikomaiDesktopMac.build/Debug/MikomaiDesktopMac-p.build/DerivedSources/resource_bundle_accessor.swift" \
    -o "$CHECK_DIR/check"
cp -R "$SCRATCH/out/Products/Debug/MikomaiDesktopMac_MikomaiDesktopMac.bundle" "$CHECK_DIR/"
"$CHECK_DIR/check"
printf 'Preview: %s/execution-queue.png\n' "$CHECK_DIR"
