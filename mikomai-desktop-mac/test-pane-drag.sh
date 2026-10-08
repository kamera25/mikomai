#!/bin/sh
set -eu
APP="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
CHECK_DIR="$(mktemp -d /private/tmp/mikomai-pane-drag.XXXXXX)"
trap 'rm -rf "$CHECK_DIR"' EXIT
SDK="${MIKOMAI_MACOS_SDK:-/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk}"
# Compile the production observer and policy together for native AppKit tracking.
sed '/^import MikomaiDesktopCore$/d' "$APP/Sources/MikomaiDesktopMac/PaneDragCollapse.swift" > "$CHECK_DIR/PaneDragCollapse.swift"
swiftc -sdk "$SDK" -module-cache-path /private/tmp/mikomai-clang-cache \
    "$APP/Sources/MikomaiDesktopCore/PaneResizePolicy.swift" \
    "$CHECK_DIR/PaneDragCollapse.swift" \
    "$APP/Tests/PaneDragCollapseChecks/PaneDragCollapseChecks.swift" \
    -o "$CHECK_DIR/check"
"$CHECK_DIR/check"
