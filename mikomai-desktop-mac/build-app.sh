#!/bin/sh
set -eu

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
APP="$ROOT/mikomai-desktop-mac"
DIST="$APP/dist"
BUNDLE="$DIST/MikomaiDesktopMac.app"
SDK_OVERRIDE="${MIKOMAI_MACOS_SDK:-}"
SDK_FALLBACK="/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk"
if [ -n "$SDK_OVERRIDE" ]; then
    SDK="$SDK_OVERRIDE"
else
    SDK="$(xcrun --sdk macosx --show-sdk-path 2>/dev/null || true)"
    if [ ! -d "$SDK" ]; then SDK="$SDK_FALLBACK"; fi
fi
SCRATCH="${MIKOMAI_SWIFT_SCRATCH_PATH:-/private/tmp/mikomai-swift-app-build}"
CLANG_CACHE="${MIKOMAI_CLANG_MODULE_CACHE:-/private/tmp/mikomai-clang-cache}"
SWIFTPM_CACHE="${MIKOMAI_SWIFTPM_MODULE_CACHE:-/private/tmp/mikomai-swiftpm-cache}"

if [ ! -d "$SDK" ]; then
    echo "macOS SDK not found: $SDK" >&2
    exit 1
fi

mkdir -p "$DIST" "$SCRATCH" "$CLANG_CACHE" "$SWIFTPM_CACHE"
cargo build -p mikomai-ffi --manifest-path "$ROOT/Cargo.toml"
build_swift() {
    SDKROOT="$SDK" \
    CLANG_MODULE_CACHE_PATH="$CLANG_CACHE" \
    SWIFTPM_MODULECACHE_OVERRIDE="$SWIFTPM_CACHE" \
    swift build --disable-sandbox --package-path "$APP" --scratch-path "$SCRATCH"
}
if ! build_swift; then
    if [ -n "$SDK_OVERRIDE" ] || [ "$SDK" = "$SDK_FALLBACK" ] || [ ! -d "$SDK_FALLBACK" ]; then
        echo "Swift build failed with SDK: $SDK" >&2
        exit 1
    fi
    echo "Active SDK build failed; retrying with $SDK_FALLBACK" >&2
    SDK="$SDK_FALLBACK"
    build_swift
fi
PRODUCTS="$(SDKROOT="$SDK" CLANG_MODULE_CACHE_PATH="$CLANG_CACHE" SWIFTPM_MODULECACHE_OVERRIDE="$SWIFTPM_CACHE" swift build --disable-sandbox --package-path "$APP" --scratch-path "$SCRATCH" --show-bin-path)"

STAGING="$(mktemp -d "$DIST/.MikomaiDesktopMac.XXXXXX")"
STAGED_APP="$STAGING/MikomaiDesktopMac.app"
CONTENTS="$STAGED_APP/Contents"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Frameworks" "$CONTENTS/Resources"
cp "$APP/Info.plist" "$CONTENTS/Info.plist"
cp "$PRODUCTS/MikomaiDesktopMac" "$CONTENTS/MacOS/MikomaiDesktopMac"
cp "$ROOT/target/debug/deps/libmikomai_ffi.dylib" "$CONTENTS/Frameworks/libmikomai_ffi.dylib"
cp -R "$ROOT/nw-docs" "$CONTENTS/Resources/nw-docs"
touch "$CONTENTS/Resources/.mikomai-development-bundle"
install_name_tool -id @rpath/libmikomai_ffi.dylib "$CONTENTS/Frameworks/libmikomai_ffi.dylib"
install_name_tool -change "$ROOT/target/debug/deps/libmikomai_ffi.dylib" @rpath/libmikomai_ffi.dylib "$CONTENTS/MacOS/MikomaiDesktopMac"
codesign --force --deep --sign - "$STAGED_APP"
codesign --verify --deep --strict "$STAGED_APP"
if [ -e "$BUNDLE" ]; then
    if [ ! -f "$BUNDLE/Contents/Resources/.mikomai-development-bundle" ]; then
        echo "Refusing to replace an app bundle not created by this script: $BUNDLE" >&2
        exit 1
    fi
    mv "$BUNDLE" "$STAGING/Previous.app"
fi
mv "$STAGED_APP" "$BUNDLE"
if [ -d "$STAGING/Previous.app" ]; then rm -rf "$STAGING/Previous.app"; fi
rmdir "$STAGING"
printf 'Built app: %s\n' "$BUNDLE"
