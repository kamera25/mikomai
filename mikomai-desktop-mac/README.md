# Mikomai Desktop for macOS

Native SwiftUI desktop interface backed by the shared Rust knowledge-chat flow through a C ABI. The chat workspace supports persistent conversations, and Settings selects the Markdown source directory and local search index used by Rust for subsequent questions. The device inventory supports local metadata editing and CSV exchange in the same non-secret column format as the Tauri app.

Device records use a separate Swift-only store. They do not synchronize with the Tauri encrypted connection store and cannot authenticate to or connect to devices. The native app does not yet expose Tauri's model loading and inference, MCP tools, operation approval and execution, configuration diff, scheduled watches, task audit, attachments, or interactive tool-choice flows.

## Feature parity

| Tauri workflow | macOS native status | Notes |
| --- | --- | --- |
| Local knowledge chat | Available | Uses shared Rust knowledge search; returns retrieved source text and does not run the Tauri LLM. |
| Chat sessions | Available, separate storage | Swift `UserDefaults`; does not share Tauri/SurrealDB history or agent-task records. |
| Knowledge settings | Available, separate storage | Document and index directories are saved by the native app and passed to Rust per request. |
| Device connection editor | Inventory only | Local metadata CRUD and CSV exchange; no keyring, credentials, connection test, or live device session. |
| Model selection/loading/inference | Not available | The Tauri model runtime and status events are not wired to the native app. |
| MCP tools, host suggestions, and user choices | Not available | No native MCP transport, event broker, or choice continuation yet. |
| Operation plans, approvals, and config diff | Not available | Tauri safety and execution state remains bound to its app services. |
| Scheduled watches and notifications | Not available | Tauri scheduler depends on AppHandle, managed state, and emitted events. |
| Agent task audit/history | Not available | Native chat sessions are not agent task snapshots or audit records. |
| Attachments, images, terminal, and stop | Not available | These Tauri UI and backend flows have not been ported. |

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

`run.sh` launches the development executable against the Rust library in the repository's `target/debug` directory. Use the bundle build below for a Finder-launchable local app.

## Build a local app bundle

Run `./mikomai-desktop-mac/build-app.sh` from the repository root. It creates `mikomai-desktop-mac/dist/MikomaiDesktopMac.app`, bundles the Rust FFI library and the repository's `nw-docs`, uses an executable-relative `@rpath`, and applies an ad-hoc local signature. This machine-local development bundle is not a release or distribution package. The script replaces only a previous bundle it created, and refuses to replace an unrelated app at the same path.

The C ABI is declared in `Sources/MikomaiFFI/include/mikomai_ffi.h`. `mikomai_chat_with_paths` receives the selected document and index directories for every request. Rust returns a status code and an owned UTF-8 string; Swift releases each result with `mikomai_result_free`.
