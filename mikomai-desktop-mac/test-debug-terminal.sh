#!/bin/sh
set -eu
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
APP="$ROOT/mikomai-desktop-mac"
SDK="${MIKOMAI_MACOS_SDK:-/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk}"
CHECK_DIR=/private/tmp/mikomai-debug-terminal-check
mkdir -p "$CHECK_DIR"
swiftc -sdk "$SDK" -module-cache-path "$CHECK_DIR/cache" \
 "$APP/Sources/MikomaiDesktopMac/CoreDebugTerminal.swift" \
 "$APP/Tests/DebugTerminalChecks/DebugTerminalChecks.swift" -o "$CHECK_DIR/check"
"$CHECK_DIR/check"
