#!/bin/sh
set -eu
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
APP="$ROOT/mikomai-desktop-mac"
SDK="${MIKOMAI_MACOS_SDK:-/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk}"
if [ ! -d "$SDK" ]; then SDK="$(xcrun --sdk macosx --show-sdk-path)"; fi
CHECK_DIR="$(mktemp -d /private/tmp/mikomai-local-routing.XXXXXX)"
trap 'rm -rf "$CHECK_DIR"' EXIT
sh "$APP/test-core.sh" --skip-build --filter LocalRoutingUtilityTests
swiftc -swift-version 6 -sdk "$SDK" -module-cache-path "$CHECK_DIR/cache" \
    -I /private/tmp/mikomai-swift-app-build/out/Products/Debug \
    -Xcc "-fmodule-map-file=/private/tmp/mikomai-swift-app-build/out/Intermediates.noindex/GeneratedModuleMaps/MikomaiGeneratedFFI.modulemap" \
    /private/tmp/mikomai-swift-app-build/out/Products/Debug/MikomaiDesktopCore.o \
    /private/tmp/mikomai-swift-app-build/out/Products/Debug/MikomaiBindings.o \
    "$APP/Tests/LocalRoutingChecks/LocalRoutingChecks.swift" \
    -L "$ROOT/target/debug/deps" -lmikomai_bindings \
    -Xlinker -rpath -Xlinker "$ROOT/target/debug/deps" -o "$CHECK_DIR/check"
"$CHECK_DIR/check"
