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
cat "$APP/Tests/ChatWindowChecks/ChatWindowChecks.swift" >> "$CHECK_DIR/full.swift"
swiftc -suppress-warnings -sdk "$SDK" -module-cache-path "$CHECK_DIR/cache" \
 -I "$SCRATCH/out/Products/Debug" \
 -Xcc "-fmodule-map-file=$SCRATCH/out/Intermediates.noindex/GeneratedModuleMaps/MikomaiFFI.modulemap" \
 -L "$ROOT/target/debug" -lmikomai_ffi -Xlinker -rpath -Xlinker "$ROOT/target/debug" \
 "$SCRATCH/out/Products/Debug/MikomaiDesktopCore.o" "$CHECK_DIR/full.swift" \
 "$APP/Sources/MikomaiDesktopMac/AgentProgressView.swift" \
 "$APP/Sources/MikomaiDesktopMac/ExecutionTerminalView.swift" \
 "$APP/Sources/MikomaiDesktopMac/AgentTaskWorkspace.swift" \
 "$APP/Sources/MikomaiDesktopMac/ChatComposer.swift" \
 "$APP/Sources/MikomaiDesktopMac/ConnectionsWorkspace.swift" \
 "$APP/Sources/MikomaiDesktopMac/DesktopCallbacks.swift" \
 "$APP/Sources/MikomaiDesktopMac/DesktopDiagnostics.swift" \
 "$APP/Sources/MikomaiDesktopMac/DesktopModel.swift" \
 "$APP/Sources/MikomaiDesktopMac/DesktopModels.swift" \
 "$APP/Sources/MikomaiDesktopMac/DesktopWindow.swift" \
 "$APP/Sources/MikomaiDesktopMac/MarkdownMessage.swift" \
 "$APP/Sources/MikomaiDesktopMac/MessageRow.swift" \
 "$APP/Sources/MikomaiDesktopMac/MonitoringWorkspace.swift" \
 "$APP/Sources/MikomaiDesktopMac/NetworkToolsWorkspace.swift" \
 "$APP/Sources/MikomaiDesktopMac/SettingsWorkspace.swift" \
 "$APP/Sources/MikomaiDesktopMac/SessionRow.swift" \
 "$SCRATCH/out/Intermediates.noindex/MikomaiDesktopMac.build/Debug/MikomaiDesktopMac-p.build/DerivedSources/resource_bundle_accessor.swift" \
 -o "$CHECK_DIR/check"
cp -R "$SCRATCH/out/Products/Debug/MikomaiDesktopMac_MikomaiDesktopMac.bundle" "$CHECK_DIR/"
"$CHECK_DIR/check"
