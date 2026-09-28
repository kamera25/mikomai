# Mikomai Desktop for macOS

Native SwiftUI desktop interface backed by Rust through a C ABI. Chat searches the selected Markdown corpus, then uses the loaded local GGUF model to generate an answer with recent conversation context and retrieved material. Model files stay outside the app bundle and are not downloaded automatically.

Device records use a separate Swift-only store. The app can import non-secret metadata from a user-selected Tauri `connections.json`, but never writes that file or imports credentials; imported records remain local copies and cannot authenticate to or connect to devices. The native app does not yet expose Tauri's MCP tools, operation approval and execution, configuration diff, scheduled watches, task audit, or interactive tool-choice flows.

## Feature parity

| Tauri workflow | macOS native status | Notes |
| --- | --- | --- |
| Local knowledge chat | Available | Searches Markdown with the shared Rust knowledge service, then generates a response with the selected local GGUF through `llama-cpp-2`. Recent session turns and retrieved documents are included in the prompt. |
| Chat sessions | Available, separate storage | Swift `UserDefaults`; does not share Tauri/SurrealDB history or agent-task records. |
| Knowledge settings | Available, separate storage | Document and index directories are saved by the native app and passed to Rust per request. |
| Device connection editor | Inventory with read-only import | Local metadata CRUD and CSV exchange. A selected Tauri `connections.json` is read through the shared `JsonDeviceRegistry` adapter with a 2 MiB limit; secrets are ignored, source IDs are retained for repeat-import deduplication, and the source file is never modified. No keyring, credentials, connection test, or live device session. |
| Model selection/loading/inference | Available, separate runtime | Select and load a local `.gguf` in Settings. Uses Tauri's llama.cpp binding, system prompt, and Gemma turn framing, but does not share Tauri's loaded model/state. |
| MCP tools, host suggestions, and user choices | Not available | No native MCP transport, event broker, or choice continuation yet. |
| Operation plans, approvals, and config diff | Not available | Tauri safety and execution state remains bound to its app services. |
| Scheduled watches and notifications | Not available | Tauri scheduler depends on AppHandle, managed state, and emitted events. |
| Agent task audit/history | Not available | Native chat sessions are not agent task snapshots or audit records. |
| Attachments and images | Text files available | `.txt`, `.md`, `.csv`, `.json`, `.yaml`, `.xml`, and `.log` are read as UTF-8, limited to 64 KiB per file / 128 KiB total, and passed as untrusted reference material to the model. File contents are not saved in chat history; the filename is retained. Images, PDF extraction, and attachments shared with Tauri history are not supported. |
| Stop generation | Available | Cancels token generation; prompt preparation and model loading are not interruptible. |

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

On first launch, open Settings, choose a `.gguf` file, and select **読み込む**. Model files are not bundled or downloaded. Loading a multi-gigabyte model requires sufficient memory. The native app defaults to CPU layer placement for compatibility; `MIKOMAI_N_GPU_LAYERS` can opt into Metal offload (for example, `99`) when launching from a terminal. The chosen model path is saved in native app preferences, while the loaded runtime is process-local and must be loaded again after restarting the app. Errors from model loading are shown in Settings; chat reports an explicit error if no model has been loaded.

`run.sh` launches the development executable against the Rust library in the repository's `target/debug` directory. Use the bundle build below for a Finder-launchable local app.

## Build a local app bundle

Run `./mikomai-desktop-mac/build-app.sh` from the repository root. It creates `mikomai-desktop-mac/dist/MikomaiDesktopMac.app`, bundles the Rust FFI library and the repository's `nw-docs`, uses an executable-relative `@rpath`, and applies an ad-hoc local signature. This machine-local development bundle is not a release or distribution package. The script replaces only a previous bundle it created, and refuses to replace an unrelated app at the same path.

The C ABI is declared in `Sources/MikomaiFFI/include/mikomai_ffi.h`. The app loads a selected model with `mikomai_model_load`, checks the active path with `mikomai_model_status`, and sends chat requests through `mikomai_assistant_chat` with the current document and index directories. Rust returns a status code and owned UTF-8 string; Swift releases each result with `mikomai_result_free`.
