#!/bin/sh
set -eu
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
APP="$ROOT/mikomai-desktop-mac"
SCRATCH="${MIKOMAI_SWIFT_SCRATCH_PATH:-/private/tmp/mikomai-swift-app-build}"
SDK="${MIKOMAI_MACOS_SDK:-/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk}"
CHECK_DIR=/private/tmp/mikomai-full-check
mkdir -p "$CHECK_DIR"
printf '%s' '{"modelPath":null}' > "$CHECK_DIR/settings.json"
# Compile the production window and model into the same file as the harness,
# preserving their private visibility while replacing only the app entry point.
sed '/^@main$/d' "$APP/Sources/MikomaiDesktopMac/MikomaiDesktopMac.swift" > "$CHECK_DIR/full.swift"
cat "${MIKOMAI_WINDOW_CHECK_SOURCE:-$APP/Tests/ChatWindowChecks/ChatWindowChecks.swift}" >> "$CHECK_DIR/full.swift"
set -- "$CHECK_DIR/full.swift"
for source in "$APP"/Sources/MikomaiDesktopMac/*.swift; do
    case "$source" in */MikomaiDesktopMac.swift) ;; *) set -- "$@" "$source" ;; esac
done
swiftc -suppress-warnings -sdk "$SDK" -module-cache-path "$CHECK_DIR/cache" \
 -I "$SCRATCH/out/Products/Debug" \
 -Xcc "-fmodule-map-file=$SCRATCH/out/Intermediates.noindex/GeneratedModuleMaps/MikomaiFFI.modulemap" \
 -L "$ROOT/target/debug" -lmikomai_ffi -Xlinker -rpath -Xlinker "$ROOT/target/debug" \
 "$SCRATCH/out/Products/Debug/MikomaiDesktopCore.o" "$@" \
 "$SCRATCH/out/Intermediates.noindex/MikomaiDesktopMac.build/Debug/MikomaiDesktopMac-p.build/DerivedSources/resource_bundle_accessor.swift" \
 -o "$CHECK_DIR/check"
cp -R "$SCRATCH/out/Products/Debug/MikomaiDesktopMac_MikomaiDesktopMac.bundle" "$CHECK_DIR/"
"$CHECK_DIR/check"
