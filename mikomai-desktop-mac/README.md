# Mikomai Desktop for macOS

Native SwiftUI desktop interface backed by Rust through a C ABI. Chat searches the selected Markdown corpus, then uses the loaded local GGUF model to generate an answer with recent conversation context and retrieved material. Model files stay outside the app bundle and are not downloaded automatically.

Device records use a separate Swift-only store. The app can import non-secret metadata from a user-selected Tauri `connections.json`, but never writes that file or imports credentials; imported records remain local copies and cannot authenticate to or connect to devices. The native app does not yet expose Tauri's MCP tools, operation approval and execution, configuration diff, scheduled watches, task audit, or interactive tool-choice flows.

## Feature parity

| Tauri workflow | macOS native status | Notes |
| --- | --- | --- |
| Local knowledge chat | Available (Streaming) | Searches Markdown with shared Rust knowledge service, then generates a streaming response with the selected local GGUF through `llama-cpp-2`. Recent session turns, attachments, and retrieved documents are included in prompt. |
| Chat sessions | Available, separate storage | Swift `UserDefaults`; does not share Tauri/SurrealDB history or agent-task records. |
| Knowledge settings | Available, separate storage | Document and index directories are saved by native app and passed to Rust per request. |
| Application & model settings | Full parity with auto-sync | Automatically loads and synchronizes Tauri's `settings.json` (`~/Library/Application Support/com.mikomai.agent/settings.json`). Supports history limit, temperature, repetition penalty, MCP timeout, cache expiry, IP preference, auto dry-run, serial console port/baud rate, Gemma 4 presets (E4B/12B/E2B/custom), HuggingFace cache checks, context length (`n_ctx`), max tokens (`max_gen`), prompt keep tokens, 6-worker KV cache preloading, and vision/mmproj settings. |
| Device connection editor | Inventory with Keychain & Test | Local metadata CRUD, CSV exchange, and read-only Tauri JSON import. Passwords and enable passwords are encrypted and securely stored in macOS Keychain. Individual devices support instant TCP connection tests and Ping dispatch. |
| Network diagnostics tools | Available | Dedicated workspace for TCP connection tests (with port presets & latency measurement), live streaming Ping & Traceroute, local ARP cache table inspection (with 1-click device registration), and routing table view. |
| Status bar & Native menus | Available | macOS native CommandMenu shortcuts (`Cmd+N`, `Cmd+1..4`, network actions) and bottom status bar (model status, knowledge path, device count). |
| Model selection/loading/inference | Available, separate runtime | Select and load a local `.gguf` in Settings. Uses Tauri's llama.cpp binding, system prompt, and Gemma turn framing, but does not share Tauri's loaded model/state. |
| MCP tools, host suggestions, and user choices | In progress (Phase 3) | Chat suggestions use saved native connections and Tauri recent IP settings. MCP host discovery and tool-choice flows remain unimplemented; diagnostics (Ping, Trace, ARP, Route, TCP Test) are available. |
| Operation plans, approvals, and config diff | Not available (Phase 3) | Tauri safety and execution state remains bound to its app services. |
| Scheduled watches and notifications | Not available | Tauri scheduler depends on AppHandle, managed state, and emitted events. |
| Agent task audit/history | Not available | Native chat sessions are not agent task snapshots or audit records. |
| Attachments and images | Text files available | `.txt`, `.md`, `.csv`, `.json`, `.yaml`, `.xml`, and `.log` are read as UTF-8, limited to 64 KiB per file / 128 KiB total, and passed as untrusted reference material to the model. File contents are not saved in chat history; the filename is retained. Images, PDF extraction, and attachments shared with Tauri history are not supported. |
| Stop generation | Available | Cancels token generation in real time; prompt preparation and model loading are not interruptible. |

## Swift test migration

`Tests/MikomaiDesktopCoreTests` ports the first desktop test behaviors to the shared native core. The tested logic is also called by the SwiftUI app.

| Existing desktop test area | Migrated behavior |
| --- | --- |
| `ChatInput.test.tsx` | Sending before a model is loaded, blocking empty or in-progress sends, supported text files, duplicate names, UTF-8/NUL checks, and the 64 KiB per-file / 128 KiB total boundaries. Core tests also cover prompt normalization, attachment-aware send availability, stop availability, suggestion updates/empty-result dismissal, and Escape dismissal. |
| `Sidebar.test.tsx` | Selecting a session, keeping the active ID valid after selection or deletion, creating a replacement after deleting the final session, and ignoring blank rename values. |
| Connection import and `connections/mod.rs` serialization tests | Tauri `type` alias, numeric port (as emitted by Tauri) and string port, missing IDs, duplicate IDs, invalid rows, and preserving valid rows when another row is malformed. |
| `CsvImportExport.test.tsx` and `connections/mod.rs` CSV cases | Tauri CSV headers and type aliases, ID generation and last-row-wins upsert, per-row warnings, required address fields, invalid hostname/IP/port/type and control characters, quoted fields/newlines, BOM/CRLF, and excluding credential fields from export. |
| `ConnectionSettingsPanel.test.tsx` native-app subset | Connection editor validation, inventory save/update/delete policy, credential-presence metadata (including empty-value clearing), SSH/Console selection, and serial port/baud settings persistence. macOS Keychain operations remain in the app's Keychain helper. The bulk Node DB refresh and Tauri SSH execution are backend-only and unsupported by the native FFI. |
| `suggestionModel.test.ts` and `useHostSuggestions.test.ts` | Case-insensitive hostname/local-host matching, matching recent IPs, suppressing IPs already represented by saved connections, prepending/deduplicating recent hosts, and retaining only the newest 10. The native composer offers saved connection and recent-IP candidates and saves hosts/IPs found when sending. |
| `useSettings.test.ts` | Loads populated Tauri settings with defaults for omitted keys; partial temperature updates merge into current values, and save encoding retains a complete settings payload. The native app uses this Core DTO/codec for its existing `settings.json` sync. |
| `attachmentModel.test.ts` | Image extension and MIME classification, including uppercase paths and MIME detection when the filename extension is not an image. This is classification only: the native text attachment workflow rejects image and PDF files. |
| `ipUtils.test.ts` | IPv4/IPv6 public-vs-local range and invalid-address cases. Native host suggestions use this classification to distinguish public recent IPs; Ping targets still allow private/local addresses. |
| `commandParser.test.ts` | Free-form ping host extraction, Japanese command words, size/count/DF options, and unrelated-input rejection. The pure parser contract is ported, but the native diagnostics UI intentionally stays structured (target, mode, count controls) and does not add free-form command parsing. |
| `settingsModelPresets.test.ts` | The three shipped model presets remain stable, and exact repository/filename lookup returns a preset only for known coordinates. The native settings picker now reads this tested Core catalog. |
| `timelineModel.test.ts` | Cross-platform basename extraction from Windows and POSIX paths; the native Tauri settings loader uses it to match model filenames and presets. Agent tool-event classification helpers remain out of scope because native chat messages do not carry those event types. |

The SwiftUI suggestion list is click-selected and replaces the trailing `@query`; empty results close it and Escape dismisses it. The composer uses ⌘+Enter for sending. Core policy tests cover these states and send/stop eligibility, but actual SwiftUI button clicks, keyboard dispatch, rendered states, browser textarea caret tracking, cursor restoration, arrow-key suggestion selection, Tab selection, and IME composition still need interaction-level UI tests. The Tauri hook also merges MCP-discovered hosts and resolves unknown IPv4 addresses; native suggestions currently use saved connections and configured recent IPs only. Settings tests cover shared load/merge/serialization policy, not filesystem path precedence, access failures, or cross-process changes while the native app is open. The DOM-specific assertions in `ImageModal.test.tsx` (image rendering, close button, backdrop and Escape handling) are not migrated because the native attachment workflow cannot accept or infer images. The FFI currently accepts UTF-8 text only; adding a local preview without an actual image attachment workflow would imply unsupported vision capability.

Run the native core tests with `swift test --package-path mikomai-desktop-mac`. When the active compiler and SDK do not match, this machine uses the installed 26.5 SDK and writable temporary caches:

```sh
SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk \
CLANG_MODULE_CACHE_PATH=/private/tmp/mikomai-clang-cache \
SWIFTPM_MODULECACHE_OVERRIDE=/private/tmp/mikomai-swiftpm-cache \
swift test --disable-sandbox --package-path mikomai-desktop-mac \
  --scratch-path /private/tmp/mikomai-swift-test-build
```

### Chat keyboard regression checks

Run `sh mikomai-desktop-mac/test-chat-composer.sh` from the repository root on macOS. The script compiles the production `ChatComposer.swift` together with `Tests/ChatComposerChecks/ChatComposerChecks.swift`; it does not duplicate the input implementation or require the Swift Testing plugin or Rust library. The script also builds the native core module for host-registry checks. Set `MIKOMAI_MACOS_SDK` if a specific SDK is needed.

The checks send AppKit key events to the actual chat text view and set marked text through `NSTextInputClient`. They cover Enter submission without a newline, suppression during Japanese composition (including Command/Shift modifiers), submission after composition, Shift+Enter newline insertion, Command+Enter, keypad Enter, and a disabled input. They also cover cursor-aware @ completion, Unicode offsets, suffix preservation, suggestion navigation/acceptance/dismissal, and IME priority over suggestions. The checks also mount the production input and completion presentation state in SwiftUI, covering delayed host loading, Escape dismissal, and the full completion round trip. They do not automate a real IME candidate window or model inference.

Host suggestions read non-secret names and IP addresses from Tauri's `connections.json` beside the settings file, alongside native connections and recent IPs. `MIKOMAI_CONNECTIONS_FILE` can override the registry path. This lookup does not import or modify connections or credentials. Both `@` and Japanese full-width `＠` open completion suggestions.



### Remaining test-driven port

The imported/exported data codec and validation are covered in Core. Native file-panel presentation, cancellation, read/write failures, and warning dialogs do not yet have SwiftUI interaction tests. Connection inventory CRUD, editor validation, credential metadata, and console setting serialization have Core tests; actual editor field selection, save/delete clicks, and Keychain access still need macOS UI/integration tests. Node DB bulk refresh and SSH execution require Tauri backend services absent from the native FFI and remain unsupported. The tests above are a starting slice, not full parity. Continue porting cases and implement the missing behavior in this order:

1. Chat input and settings: IME/keyboard behavior and vision rejection in `ChatInput`; native file-panel cancellation/read errors; and image/PDF handling only after the native FFI has a real multimodal or extraction API. Port `ImageModal.test.tsx` interaction assertions only with that user-facing preview workflow.
2. Device connections: editor/Console credential interactions and CSV cases (`ConnectionSettingsPanel`), followed by bulk Node DB refresh. Native local inventory and metadata import do not implement these service-backed actions.
3. Agent interaction and safety: user choice queues (`useQuestionQueue`), hash-bound plan approval and execution (`ConfigDiffPanel`), and task selection/resume/audit (`TaskAuditPanel`). Keep the native implementation's approval and execution state explicit before enabling device changes.
4. MCP and scheduled work: listener lifecycle (`useMcpListeners`, `mcpListenerState`), host suggestion updates, watches, and notifications. Validate mocks separately from real device or server access.
5. UI parity and presentation: sidebar event/agent-step rendering, Terminal highlighting, status bar, settings controls, modal dismissal, and pane resizing. The native SwiftUI UI needs interaction-level checks in addition to Core unit tests.

The existing Tauri Rust integration and harness tests also cover device operations and app-managed state. Port those contracts alongside the corresponding native feature; passing CLI chat or Core tests does not verify UI actions, MCP calls, or device access.

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

For a production-window regression check, build the app first, then run:

```sh
sh mikomai-desktop-mac/test-chat-window.sh
```

This mounts the actual `DesktopWindow` and `DesktopModel`, verifies that bare half-width and full-width @ produce an Enter-selectable localhost candidate, and saves a rendered window to `/private/tmp/mikomai-full-check/window.png`. The completion policy checks also verify empty-query results with zero devices, registered Tauri hosts, and recent IPs.
