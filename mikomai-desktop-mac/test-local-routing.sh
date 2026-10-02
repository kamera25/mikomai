#!/bin/sh
set -eu
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
APP="$ROOT/mikomai-desktop-mac"
SDK="${MIKOMAI_MACOS_SDK:-/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk}"
if [ ! -d "$SDK" ]; then SDK="$(xcrun --sdk macosx --show-sdk-path)"; fi
CHECK_DIR="$(mktemp -d /private/tmp/mikomai-local-routing.XXXXXX)"
trap 'rm -rf "$CHECK_DIR"' EXIT
cargo build -p mikomai-ffi --manifest-path "$ROOT/Cargo.toml"
swiftc -swift-version 6 -sdk "$SDK" -module-cache-path "$CHECK_DIR/cache" \
    -import-objc-header "$APP/Sources/MikomaiFFI/include/mikomai_ffi.h" \
    "$APP/Sources/MikomaiDesktopCore/LocalRoutingUtility.swift" \
    "$APP/Tests/LocalRoutingChecks/LocalRoutingChecks.swift" \
    -L "$ROOT/target/debug/deps" -lmikomai_ffi \
    -Xlinker -rpath -Xlinker "$ROOT/target/debug/deps" -o "$CHECK_DIR/check"
"$CHECK_DIR/check"
