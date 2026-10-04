#!/bin/sh
set -eu

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
APP="$ROOT/mikomai-desktop-mac"
DIST="$APP/dist"
BUNDLE="$DIST/Mikomai.app"
SDK_OVERRIDE="${MIKOMAI_MACOS_SDK:-}"
SDK_FALLBACK="/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk"
if [ -n "$SDK_OVERRIDE" ]; then
    SDK="$SDK_OVERRIDE"
elif [ -d "$SDK_FALLBACK" ]; then
    # Prefer the toolchain version used by the validated Swift build when it is installed.
    SDK="$SDK_FALLBACK"
else
    SDK="$(xcrun --sdk macosx --show-sdk-path 2>/dev/null || true)"
    if [ ! -d "$SDK" ]; then
        echo "macOS SDK not found: $SDK" >&2
        exit 1
    fi
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
    # SwiftPM's Xcode backend can record the deployment target as the SDK
    # version. AppKit uses the linked SDK to enable the modern window controls.
    SDK_VERSION="$(xcrun --sdk "$SDK" --show-sdk-version)"
    DEPLOYMENT_VERSION="$(/usr/libexec/PlistBuddy -c 'Print :LSMinimumSystemVersion' "$APP/Info.plist")"
    SDKROOT="$SDK" \
    CLANG_MODULE_CACHE_PATH="$CLANG_CACHE" \
    SWIFTPM_MODULECACHE_OVERRIDE="$SWIFTPM_CACHE" \
    swift build --disable-sandbox --disable-experimental-prebuilts --sdk "$SDK" --package-path "$APP" --scratch-path "$SCRATCH" \
        -Xlinker -platform_version -Xlinker macos \
        -Xlinker "$DEPLOYMENT_VERSION" -Xlinker "$SDK_VERSION"
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
PRODUCTS="$(SDKROOT="$SDK" CLANG_MODULE_CACHE_PATH="$CLANG_CACHE" SWIFTPM_MODULECACHE_OVERRIDE="$SWIFTPM_CACHE" swift build --disable-sandbox --disable-experimental-prebuilts --sdk "$SDK" --package-path "$APP" --scratch-path "$SCRATCH" --show-bin-path)"

STAGING="$(mktemp -d "$DIST/.Mikomai.XXXXXX")"
STAGED_APP="$STAGING/Mikomai.app"
CONTENTS="$STAGED_APP/Contents"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Frameworks" "$CONTENTS/Resources"
cp "$APP/Info.plist" "$CONTENTS/Info.plist"
cp "$APP/Sources/MikomaiDesktopMac/Resources/AppIcon.icns" "$CONTENTS/Resources/AppIcon.icns"
cp -R "$PRODUCTS/MikomaiDesktopMac_MikomaiDesktopMac.bundle" "$CONTENTS/Resources/"
cp "$PRODUCTS/MikomaiDesktopMac" "$CONTENTS/MacOS/Mikomai"
cp "$ROOT/target/debug/deps/libmikomai_ffi.dylib" "$CONTENTS/Frameworks/libmikomai_ffi.dylib"
cp -R "$ROOT/nw-docs" "$CONTENTS/Resources/nw-docs"
cp "$ROOT/mikomai-core/assets/bin/netmiko_wrapper-macos-arm64" "$CONTENTS/Resources/netmiko_wrapper"
mkdir -p "$CONTENTS/Resources/network"
cp "$ROOT/mikomai-core/assets/network/netmiko_wrapper.py" "$CONTENTS/Resources/network/netmiko_wrapper.py"
cp "$ROOT/mikomai-core/assets/network/netmiko_patches.py" "$CONTENTS/Resources/network/netmiko_patches.py"
cp "$ROOT/mikomai-core/assets/network/config_helper.py" "$CONTENTS/Resources/network/config_helper.py"
cp "$ROOT/mikomai-core/assets/network/nwdiag_wrapper.py" "$CONTENTS/Resources/network/nwdiag_wrapper.py"
cp -R "$ROOT/mikomai-core/assets/templates" "$CONTENTS/Resources/templates"
chmod 755 "$CONTENTS/Resources/netmiko_wrapper"
touch "$CONTENTS/Resources/.mikomai-development-bundle"
install_name_tool -id @rpath/libmikomai_ffi.dylib "$CONTENTS/Frameworks/libmikomai_ffi.dylib"
install_name_tool -change "$ROOT/target/debug/deps/libmikomai_ffi.dylib" @rpath/libmikomai_ffi.dylib "$CONTENTS/MacOS/Mikomai"
# macOS 27 hides the ARP cache from apps without the Network Topology
# Observation capability. It requires an authorised signing identity; adding
# the restricted entitlement to an ad-hoc signature prevents the app launching.
SIGN_IDENTITY="${MIKOMAI_CODESIGN_IDENTITY:--}"
if [ "$SIGN_IDENTITY" = "-" ]; then
    codesign --force --deep --sign - "$STAGED_APP"
else
    if [ -n "${MIKOMAI_PROVISIONING_PROFILE:-}" ]; then
        cp "$MIKOMAI_PROVISIONING_PROFILE" "$CONTENTS/embedded.provisionprofile"
    fi
    codesign --force --sign "$SIGN_IDENTITY" "$CONTENTS/Frameworks/libmikomai_ffi.dylib"
    codesign --force --sign "$SIGN_IDENTITY" --entitlements "$APP/NetworkTopology.entitlements" "$STAGED_APP"
fi
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
