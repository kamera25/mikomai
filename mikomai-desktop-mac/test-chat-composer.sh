#!/bin/sh
set -eu

APP="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
TEST_DIR="$(mktemp -d "${TMPDIR:-/private/tmp}/mikomai-chat-composer.XXXXXX")"
trap 'rm -rf "$TEST_DIR"' EXIT HUP INT TERM

SDK_OVERRIDE="${MIKOMAI_MACOS_SDK:-${SDKROOT:-}}"
SDK="${SDK_OVERRIDE:-$(xcrun --sdk macosx --show-sdk-path)}"
SDK_FALLBACK="/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk"

compile_checks() {
    swiftc -sdk "$SDK" -module-cache-path "$TEST_DIR/cache" \
        -emit-library -emit-module -module-name MikomaiDesktopCore \
        "$APP"/Sources/MikomaiDesktopCore/*.swift \
        -emit-module-path "$TEST_DIR/MikomaiDesktopCore.swiftmodule" \
        -o "$TEST_DIR/libMikomaiDesktopCore.dylib" || return $?
    swiftc -sdk "$SDK" -module-cache-path "$TEST_DIR/cache" \
        -I "$TEST_DIR" -L "$TEST_DIR" -lMikomaiDesktopCore \
        -Xlinker -rpath -Xlinker "$TEST_DIR" \
        "$APP/Sources/MikomaiDesktopMac/ChatComposer.swift" \
        "$APP/Tests/ChatComposerChecks/ChatComposerChecks.swift" \
        -o "$TEST_DIR/checks"
}

if ! compile_checks; then
    if [ -n "$SDK_OVERRIDE" ] || [ "$SDK" = "$SDK_FALLBACK" ] || [ ! -d "$SDK_FALLBACK" ]; then
        exit 1
    fi
    SDK="$SDK_FALLBACK"
    compile_checks
fi

"$TEST_DIR/checks"
