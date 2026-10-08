# query_state / diff_state 実装・検証記録（2026-10-08）

API仕様と入出力例は [保存済み状態API](stored-state.md) を参照。

## 実装

既存SurrealDB observation履歴をリソース単位のSnapshotとして再利用。
Coreの純粋演算とAdapterの保存参照を分離し、CLIとLLM Agentから同じサービスを実行する。
queryは等値ANDフィルタ・フィールド選択・安定順序・件数上限、diffは既存identityによる
追加/削除/構造的変更・nullと欠落の区別・順序無視の明示規則を実装した。
取得範囲とバージョンの不一致、不完全なSnapshotのdiffを拒否する。
追加依存ライブラリ、独自DSL、別の保存基盤はない。

## 変更ファイル

| ファイル | 変更 |
| --- | --- |
| `mikomai-core/src/network/state.rs` | 新規。入出力型、JSON Schema、純粋query/diff、8件の単体テスト |
| `mikomai-core/src/network/mod.rs` | 状態モジュールの公開 |
| `mikomai-core/src/network/interface_state.rs` | 収集失敗/空出力もraw-only観測として保存 |
| `mikomai-core/src/network/arp_state.rs` | 同上。既存get_stateの公開入出力は維持 |
| `mikomai-core/src/tool_kind.rs` | read-onlyプリミティブ登録 |
| `mikomai-core/src/planner.rs` | 状態selector/filter/fields/limitの入力スキーマ。歴史上の機器名も許可 |
| `mikomai-core/src/agent.rs` | 保存済み状態の公開契約と呼出し方をPlannerへ提示 |
| `mikomai-core/src/dispatch.rs` | 明示名/自然な保存済み状態操作をAgentへ渡す。説明依頼との区別テスト |
| `crates/mikomai-adapters/src/state.rs` | 新規。既存カタログ再利用、観測参照、出力bytes上限、5件のテスト |
| `crates/mikomai-adapters/src/lib.rs` | 状態サービスの公開 |
| `crates/mikomai-adapters/src/portable_device.rs` | 公開read-only descriptorへの追加 |
| `crates/mikomai-adapters/src/portable_graph.rs` | latest/明示IDの観測参照、scope分離、完全性検証 |
| `crates/mikomai-adapters/src/store_broker.rs` | 同じ参照を既存同一ユーザーbrokerから実行 |
| `crates/mikomai-app/src/lib.rs` | Agent入口。機器接続/資格情報/推論を使わないことを検証するテスト |
| `crates/mikomai-app/src/native_execution.rs` | CLI/native呼出し共通入口 |
| `crates/mikomai-cli/src/main.rs` | query-state/diff-state、保存済み状態chatのAgentルーティング |
| `doc/stored-state.md` | 新規。契約、例、意味規則、制限と拡張候補 |
| `doc/agent-architecture.md` | 状態APIへの参照 |
| `doc/mikomai-cli.md` | 新コマンドの案内 |
| `doc/stored-state-verification.md` | この検証記録 |

## 自動検証

```sh
cargo test -p mikomai-core -p mikomai-adapters -p mikomai-app -p mikomai-cli --lib --bins
```

合格: Core 150、Adapter 48、App 60、CLI 6、合計264件。既存の条件付きテスト7件はignored。
ログ: `/tmp/state-tests-final2.log`。
保存層broker/既存loopback試験のためsandbox外で実行した。

query: 条件一致/不一致、不存在Snapshot、projection、件数上限、partial/unavailable、
未知フィールド・不正条件、scope分離を確認。
diff: 変更なし、フィールド変更、追加、削除、複数変更、コレクション順序、
null/欠落、部分取得、指定エラー、バージョン不一致、重複identity、返却上限を確認。

新しい保存層試験は一時DBのみを使い、実機とLLMを使用しない。
失敗したInterface収集が保存され、その最新観測をbroker経由でもunavailableとして返すことを確認。
Agent実行入口はquery/diffでtransport/inferenceを呼ぶと即失敗するテストで検証。

ビルド: `cargo check -p mikomai-cli`、最終CLIビルド/テストコンパイルは合格。
新規Rustファイルの `rustfmt --check --edition 2021`、`git diff --check` は合格。

通常Clippyは既存 `transfer.rs` の `while_immutable_condition`、既存FFI関数の
`not_unsafe_ptr_arg_deref` により未合格。既存箇所は変更していない。
以下の限定的な除外を付けた追加解析は終了コード0。既存warningsは残るが追加箇所のwarningsは解消。

```sh
cargo clippy -p mikomai-core -p mikomai-adapters -p mikomai-app -p mikomai-cli --all-targets -- -A clippy::while_immutable_condition -A clippy::not_unsafe_ptr_arg_deref
```

ログ: `/tmp/state-clippy.log`, `/tmp/state-clippy-relaxed.log`, `/tmp/state-clippy-verified.log`。

## CLI

直接query-stateで `R1/interfaces`, fields=name,status, limit=1を実行し、
`results: [{"name":"eth1","status":"down"}]`, availability=completeを確認。終了コード0。
同じlatest同士のdiff-stateはchanges=[], total_changes=0。終了コード0。
ログ: `/tmp/state-cli-query.json`, `/tmp/state-cli-diff.json`（stderrは同名の `.stderr`）。

必須基本質問 `npm run --silent cli -- chat 'F220のVLAN設定方法を教えて' --debug-jsonl` を実行。
通常設定の実モデル経路は199行を全行JSON解析し、最後のcore_response.status=0と非空回答を確認。
interface GigaEthernet/vlan-id/channel-groupのテンプレートはF220 Access VLAN資料と一致。
ログ: `/tmp/state-chat-basic.jsonl`, `/tmp/state-chat-basic.stderr`。
最終変更後のテストDBではモデル未設定Markdown経路も7行を全行解析してstatus=0を確認。
ログ: `/tmp/state-chat-basic-final.jsonl`, `/tmp/state-chat-basic-final.stderr`。

変更対応chatは通常設定と同じローカルGGUFモデルを `MIKOMAI_MODEL_PATH` で指定し、
テスト用一時DBを `MIKOMAI_GRAPH_DB_PATH` で指定して実行。
query_stateでlatest/R1/interfaces、scope=all、filter=name:eth1、fields=name,status、limit=1を依頼した。
29行を全行JSON解析し、Agent dispatch、モデルのOBSERVE決定、query_state観測1件、
完全一致した引数、results=name:eth1/status:downのみ、実Snapshot ID、モデルのFINISH、
最終core_response.status=0と同じ値を含む回答を確認。終了コード0。
機器全状態は観測として返さず、選択した2フィールドとSnapshotメタデータだけをLLMへ渡した。
ログ: `/tmp/state-chat-agent.jsonl`, `/tmp/state-chat-agent.stderr`。
初回のWorker経路でコマンド例だけを返した実行は機能合格とせず、CLIのAgent接続を修正して再検証した。

## 制限・未検証と拡張候補

リソース単位の観測Snapshotで、全ネットワークの原子的/不変Snapshotではない。
旧/不完全な観測や取得範囲違いのdiffは拒否する。旧routes/NDP/configは対象外。
nested配列はInterfaceアドレス以外、順序の意味が未定義のため値として比較する。
実モデルでのdiff_state選択は未検証（Core/Adapter/共通Agent入口/直接CLIは検証済み）。
既存ignored試験、Swift UI操作、実機取得/設定変更は今回の試験対象外。
次の候補は履歴一覧、cursorページング、明示的完全性/失敗原因、不変Snapshot、
version移行、既存Graph関係を辿る限定的なjoin。独自DSLやGraphDB追加は現段階で不要。
