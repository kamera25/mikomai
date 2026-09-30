#!/bin/sh
set -eu

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
APP="$ROOT/mikomai-desktop-mac"
SDK_OVERRIDE="${MIKOMAI_MACOS_SDK:-}"
SDK_FALLBACK="/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk"
if [ -n "$SDK_OVERRIDE" ]; then
    SDK="$SDK_OVERRIDE"
elif [ -d "$SDK_FALLBACK" ]; then
    # Use the same SDK version as the validated build-app path when available.
    SDK="$SDK_FALLBACK"
else
    SDK="$(xcrun --sdk macosx --show-sdk-path 2>/dev/null || true)"
fi
if [ ! -d "$SDK" ]; then
    echo "macOS SDK not found: $SDK" >&2
    exit 1
fi
SCRATCH="${MIKOMAI_SWIFT_SCRATCH_PATH:-/private/tmp/mikomai-swift-app-build}"
CLANG_CACHE="${MIKOMAI_CLANG_MODULE_CACHE:-/private/tmp/mikomai-clang-cache}"
SWIFTPM_CACHE="${MIKOMAI_SWIFTPM_MODULE_CACHE:-/private/tmp/mikomai-swiftpm-cache}"
mkdir -p "$SCRATCH" "$CLANG_CACHE" "$SWIFTPM_CACHE"

swift_environment() {
    SDKROOT="$SDK" \
    CLANG_MODULE_CACHE_PATH="$CLANG_CACHE" \
    SWIFTPM_MODULECACHE_OVERRIDE="$SWIFTPM_CACHE" \
    swift "$@"
}

cargo build -p mikomai-ffi
swift_environment build --disable-sandbox --package-path "$APP" --scratch-path "$SCRATCH"
MIKOMAI_DOCS_DIR="$ROOT/nw-docs" \
MIKOMAI_NETMIKO_WRAPPER="$ROOT/mikomai-core/assets/bin/netmiko_wrapper-macos-arm64" \
MIKOMAI_ASSETS_DIR="$ROOT/mikomai-core/assets" \
MIKOMAI_PYTHON="$ROOT/venv/bin/python" \
DYLD_LIBRARY_PATH="$ROOT/target/debug${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}" \
swift_environment run --disable-sandbox --package-path "$APP" --scratch-path "$SCRATCH" MikomaiDesktopMac
