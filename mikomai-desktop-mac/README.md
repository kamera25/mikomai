# Mikomai Desktop for macOS

Native SwiftUI chat interface backed by the Rust knowledge-chat flow through a C ABI. The existing Tauri desktop app is unchanged.

This first native version searches the local Markdown knowledge base only. Device connections, MCP operations, the Tauri UI, and its LLM inference are not exposed through this app yet.

## Requirements

- macOS 13 or later
- Rust toolchain
- Swift 6 toolchain with a compatible macOS SDK (provided by Xcode or Command Line Tools)

## Run

From the repository root:

```sh
./mikomai-desktop-mac/run.sh
```

The script first uses the active Swift toolchain and SDK as configured. If that build fails, it tries the Command Line Tools macOS 26.5 SDK with writable caches under `/private/tmp` and disables SwiftPM's subprocess sandbox. Override the fallback with `MIKOMAI_MACOS_SDK`, `MIKOMAI_SWIFT_SCRATCH_PATH`, `MIKOMAI_CLANG_MODULE_CACHE`, and `MIKOMAI_SWIFTPM_MODULE_CACHE`. It builds `mikomai-ffi`, points the app at this repository's `nw-docs`, and launches the SwiftUI app. `MIKOMAI_KNOWLEDGE_DIR` can choose the local index directory.

This is a development launch only. The executable currently links to the Rust library in the repository's `target/debug` directory; packaging and signing a portable `.app` bundle are not configured.

The C ABI is declared in `Sources/MikomaiFFI/include/mikomai_ffi.h`. Rust returns a status code and an owned UTF-8 string; Swift must release each result with `mikomai_result_free`.
