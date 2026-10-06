# 段階3: Rust 保存サービス

2026-10-06。Windows/Vulkan/WinUIを除外して、残っていた保存サービスから実装を進めた。

## 実装

- `MikomaiService` がインスタンスごとにDBとRuntimeを所有し、明示したパスで独立したサービスを作成できる。
- graph/RAGと同じSurrealDB接続へ `sessions:current`、`connections:current`、`settings:current` を追加。各レコードは `schema_version: 1` とJSON payloadを持ち、単一UPSERTで保存する。未知のスキーマと不正な既存payloadは読み込み・上書きを拒否する。
- セッションと選択中IDを一つのsnapshotに保存。接続情報はメタデータのみとし、パスワード等の秘密フィールドを拒否する。無効な接続情報と未知のcollectionも拒否する。
- SwiftのUserDefaultsによるセッション・接続・モデル/資料/ナレッジパスの保存、設定JSONの保存を廃止。Swiftの `NativePersistence` は一時C ABI経由でRustサービスを呼ぶ。UniFFIへの移行は別段階。
- DB読込みに失敗した場合、UIは保存を停止する。空の状態で既存データを上書きしない。保存失敗はUIへ表示し、旧UserDefaults・設定JSONは取り込まず削除もしない。
- CLIも旧設定JSONの自動探索を廃止。現在のDBの設定または明示した `MIKOMAI_MODEL_PATH` を使う。DBエラーはJSONLの `core_response` と終了1で返す。
- 段階1で移動したfixtureに追従していなかったadapterテストの画像参照を修正した。

## 検証

事前条件: Swift→Rust→DBで日本語本文・UUID・日時・接続情報・パス設定を保存できること、別プロセスで再読込みできること、秘密値と未知スキーマを拒否して既存内容を保つこと、DBを開けない場合に失敗を返すこと、F220の必須CLI応答が資料に一致すること。

- Rust app 52件・CLI 6件・FFI 2件が合格。adapterの保存テスト1件も合格。実モデル専用5件はignore。段階1で変更前でも再現した資料検索fixtureの `c_abi_reports_errors_answers` 1件は分離しており、修正済みとは扱わない。
- 指定MacOSX26.5 SDKでmacOSアプリをビルド。Swiftテスト96件が合格（別プロセス再読込み用1件は通常実行でskip）。同じ一時DBを別プロセスで開き直したテスト1件も合格。GUIの画面操作は未検証。
- 開けない一時DBを指定したCLIは終了1、stdout全2行がJSON、最終 `core_response.payload.status == 1`。DB失敗を正常な回答で隠さない。
- F220質問は実GGUF/E5・一時DBを使用。検証用モデルは `MIKOMAI_MODEL_PATH` で明示し、旧設定をDBに取り込んでいない。終了0、stdout全438行をJSONとして解析し、reference_context・llm_request/response・最後のcore_response.status 0を確認。Access/Trunk VLANのGigaEthernet・vlan-id・channel-groupと両資料の出典が一致し、Ciscoのswitchport手順は含まれない。解析結果は `/tmp/mikomai-storage-verification.json`。

ログ: `/tmp/mikomai-storage-rust.log`、`/tmp/mikomai-storage-adapter-final.log`、`/tmp/mikomai-storage-cli-final-tests.log`、`/tmp/mikomai-storage-swift-final.log`、`/tmp/mikomai-storage-reopen.log`、`/tmp/mikomai-storage-cli-failure.jsonl`、`/tmp/mikomai-storage-vlan-final.jsonl`。

## 残作業と制約

段階3全体と段階4–6はまだ完了していない。tasks/operations/approvals/watches/auditは従来のファイル保存が残る。Rust keyring、常駐ワーカー・期限/中断/Unknown、Swift内の操作実行・プロセス管理、Scheduler/InferenceQueue/DeviceLockManager、TaskEvent/Query、UniFFIと共通fixture契約テストを続ける必要がある。今回のSwift/Rust保存テストは段階6の共通契約テストではない。

CLIは既定で専用の組込みDBを使用する。GUI/CLIの単一正本と同時起動を両立するサービス接続は未実装で、RocksDBを同時に二重オープンする構成にはしていない。`MIKOMAI_GRAPH_DB_PATH` による明示パス指定は可能だが、複数プロセスで同時に開くことはできない。

Windows/Vulkan/WinUI、実機への変更、実KeychainのRust移行は検証対象にしていない。
