# mikomai-app

Application orchestration shared by the native bridge and CLI. This crate depends on core and adapters; it does not depend on
`mikomai-ffi` or export C symbols.

## Stage 1

`MikomaiService` owns approval plans, execution claims, pending agent tasks, RAG
index state, the portable graph, operation audit, Watch state, backend selection,
and one lazy Tokio runtime. Graph/RAG, Agent, Watch, and approved network
operations all use that runtime. The legacy bridge uses a single application
service instance so existing callers continue to share their state.

`mikomai-ffi` only forwards the existing C ABI and re-exports the Rust helpers
used by the CLI. Callback types, pointer-based compatibility functions, result
allocation/freeing, and Swift transport adapters remain here temporarily; they
will be replaced as the later worker and UniFFI stages introduce new contracts.
The public service type is the state owner, not yet the Command/Event/Query API.
Creating a service does not switch the legacy bridge to that instance.

Stage 2 switches CLI model, chat, ARP and TCP calls directly to `mikomai-app`;
its dependency graph no longer includes `mikomai-ffi`. Existing JSONL records
and routing behavior are preserved. Scheduler, InferenceQueue,
DeviceLockManager, new persistence, native UI migration, and legacy import
removal belong to later stages. Existing persistence behavior is preserved.

## Verification

Run `cargo test -p mikomai-app -p mikomai-ffi -p mikomai-cli -- --test-threads=1`.
Tests migrated from FFI exercise approval/replay protection, async FTP/TFTP,
Watch, Agent, RAG and callback behavior. The FFI integration test additionally
checks that bridge and direct application calls see the same approval state and
that results remain safe to free across the crate boundary.

Always also run `npm run --silent cli -- chat "F220のVLAN設定方法を教えて" --debug-jsonl`,
and parse every stdout line through the final `core_response`.

See [stage 1 verification](../../doc/mikomai-app-stage1.md) for results and a
pre-existing reference-search test failure reproduced on the original source.

See [stage 2 verification](../../doc/mikomai-app-stage2.md) for the direct CLI
dependency change and JSONL FastRouter/Agent-mode/RAG verification.

Stage 3 moves session operations, connection policies, catalogs and dry-run
validation into `native_features`. Swift uses a temporary JSON policy envelope
through the compatibility bridge. Old data imports are removed, but persistence
consolidation and worker management remain pending. See [stage 3](../../doc/mikomai-app-stage3.md).
