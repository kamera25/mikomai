#!/bin/sh
set -eu

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
APP="$ROOT/mikomai-desktop-mac"
SDK="${MIKOMAI_MACOS_SDK:-/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk}"
if [ ! -d "$SDK" ]; then
    SDK="$(xcrun --sdk macosx --show-sdk-path)"
fi
SCRATCH="${MIKOMAI_SWIFT_SCRATCH_PATH:-/private/tmp/mikomai-swift-app-build}"
CLANG_CACHE="${MIKOMAI_CLANG_MODULE_CACHE:-/private/tmp/mikomai-clang-cache}"
SWIFTPM_CACHE="${MIKOMAI_SWIFTPM_MODULE_CACHE:-/private/tmp/mikomai-swiftpm-cache}"
mkdir -p "$SCRATCH" "$CLANG_CACHE" "$SWIFTPM_CACHE"

cargo build -p mikomai-ffi --manifest-path "$ROOT/Cargo.toml"
# Match the application's SDK and caches. Compile the macro dependency from
# source so testing does not depend on downloadable prebuilt artifacts.
SDKROOT="$SDK" \
CLANG_MODULE_CACHE_PATH="$CLANG_CACHE" \
SWIFTPM_MODULECACHE_OVERRIDE="$SWIFTPM_CACHE" \
swift test --disable-sandbox --disable-experimental-prebuilts \
    --package-path "$APP" --scratch-path "$SCRATCH" "$@"
