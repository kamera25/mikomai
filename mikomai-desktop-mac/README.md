# Mikomai for macOS

SwiftUIデスクトップアプリはRustの `mikomai-core` / `mikomai-adapters` をC FFI経由で利用します。SwiftチャットはローカルGGUFによるWorker回答と、LLM planner・device tools・複数stepのportable AgentLoopを使用します。Plannerには会話履歴、添付テキスト、検索資料を渡し、Agentの内部イベントや生の機器出力は通常の回答として表示しません。

## Agentとネットワーク操作

AgentはFFI allow-list経由で疎通確認、route/IP情報、serial port列挙、`get_state`、`fetch_config`、`fetch_routing`、`fetch_arp`、`network_show`、packet解析/準備/安全確認、NW図、Cisco設定validate/convert、SurrealDB graph/MAC lookup、ベクトルRAG検索を利用します。機器調査はSwift callbackがmacOS Keychainから資格情報を取り出し、Netmiko runnerを呼び出します。秘密情報はplanner schemaやLLM promptへ渡さず、取得した設定出力もredactしてから共有します。

設定変更、serial console送信、FTP/TFTP file transferはhash付きOperationPlanを作成し、Swiftの確認・承認UIに送ります。実行は承認後だけ可能で、FFIのplan hash gate、開始済み状態確認、結果確定を通ります。資格情報はSwift Keychainから必要なFTP接続に限って渡します。Ask-user choiceは候補を表示し、返信を同じpending Agent taskへ続けます。NW図生成はSVG artifactとして保存・表示します。

React/Tauri UI/runtimeとIPC、専用MCP server process群を削除しました。旧Scheduler/Watch/TaskAuditの画面レイアウトは再現していませんが、定期実行、CPU監視、通知、run history、agent task履歴・再開、操作監査は「監視・タスク履歴」workspaceへ移しています。旧 `watches.json` とtask event JSONはSwift版Application Supportへ一度だけ取り込みます。agentの登録済みtool契約はportable FFI/Swift callback/承認済みadapterへ移しています。古いtool名と現在の実行経路は [`doc/tauri-retirement.md`](../doc/tauri-retirement.md) に記録しています。

## 設定と起動

設定は `~/Library/Application Support/MikomaiDesktopMac/settings.json` に保存します。既存インストールのsettingsは初回読込時に一度だけimportし、その後はSwift-native保存先を使います。接続情報はSwift側のローカル保存、パスワード類はmacOS Keychainに保存します。GGUFモデルは同梱せず、設定画面から選んで読み込みます。RAGには旧SurrealDB pathの `rag_chunk` を再利用し、Multilingual E5-Large modelを初回利用時にFastEmbed cacheへ取得します。

監視定義と履歴は `~/Library/Application Support/MikomaiDesktopMac/watches.json` に、Agent task event/evidenceは同じディレクトリの `agent-events/` に保存します。操作監査は `audit/operations.ndjson` に記録します。旧Tauri監視・task auditのファイルがある場合は初回起動時に不足分だけ取り込みます。

リポジトリから起動する場合は `./mikomai-desktop-mac/run.sh`、配布appを作る場合は `./mikomai-desktop-mac/build-app.sh` を実行します。Netmiko wrapperの共有Python資産とmacOS arm64 sidecarは `mikomai-core/assets/` 配下にあります。

CLIは同じFFI推論器を使います。`MIKOMAI_MODEL_PATH` を指定するか、Swift-native/既存設定にGGUF pathを設定して、`npm run cli -- chat "F220のVLAN設定方法を教えて"` のように実行します。モデルが見つからない時は非生成ナレッジ応答を行うため、LLM回答を検証する場合はモデルを設定してください。

## 検証

```bash
cargo test --workspace
cargo build -p mikomai-ffi
./mikomai-desktop-mac/test-core.sh
npm run cli -- chat "F220のVLAN設定方法を教えて"
```

Swiftテストは `test-core.sh` から標準の `swift test` ランナーを実行します。アプリと同じmacOS SDK・キャッシュを使い、Testing 6.2系を固定して、対応Command Line Toolsに存在しない `_TestingInterop` への依存を避けます。特定のテストだけ実行する場合は `./mikomai-desktop-mac/test-core.sh --filter AgentProgressTests` のように指定できます。

UIとSwift callbackの確認は、`test-core.sh` の後に `./mikomai-desktop-mac/test-execution-queue.sh` を実行できます。一時設定・保存先を使い、入力と送信待機列、localhostへのping・traceroute結果を検証します。

Swift unit testsおよびRust fake transportで承認・planner・tool結果を検証します。実機SSH、モデルごとの生成品質、物理装置に対する変更適用はそれぞれの利用環境で追加確認してください。
