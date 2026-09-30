#!/bin/sh
set -eu

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
APP="$ROOT/mikomai-desktop-mac"
cargo build -p mikomai-ffi

if swift build --package-path "$APP"; then
    MIKOMAI_DOCS_DIR="$ROOT/nw-docs" \
    MIKOMAI_NETMIKO_WRAPPER="$ROOT/mikomai-desktop/src-tauri/binaries/netmiko_wrapper-aarch64-apple-darwin" \
    DYLD_LIBRARY_PATH="$ROOT/target/debug${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}" \
    swift run --package-path "$APP" MikomaiDesktopMac
    exit $?
fi

SDK="${MIKOMAI_MACOS_SDK:-/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk}"
if [ ! -d "$SDK" ]; then
    echo "Swift build failed, and the compatible SDK fallback was not found: $SDK" >&2
    exit 1
fi

SCRATCH="${MIKOMAI_SWIFT_SCRATCH_PATH:-/private/tmp/mikomai-swift-build}"
CLANG_CACHE="${MIKOMAI_CLANG_MODULE_CACHE:-/private/tmp/mikomai-clang-cache}"
SWIFTPM_CACHE="${MIKOMAI_SWIFTPM_MODULE_CACHE:-/private/tmp/mikomai-swiftpm-cache}"
mkdir -p "$SCRATCH" "$CLANG_CACHE" "$SWIFTPM_CACHE"

swift_environment() {
    SDKROOT="$SDK" \
    CLANG_MODULE_CACHE_PATH="$CLANG_CACHE" \
    SWIFTPM_MODULECACHE_OVERRIDE="$SWIFTPM_CACHE" \
    swift "$@"
}

swift_environment build --disable-sandbox --package-path "$APP" --scratch-path "$SCRATCH"
MIKOMAI_DOCS_DIR="$ROOT/nw-docs" \
MIKOMAI_NETMIKO_WRAPPER="$ROOT/mikomai-desktop/src-tauri/binaries/netmiko_wrapper-aarch64-apple-darwin" \
DYLD_LIBRARY_PATH="$ROOT/target/debug${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}" \
swift_environment run --disable-sandbox --package-path "$APP" --scratch-path "$SCRATCH" MikomaiDesktopMac
