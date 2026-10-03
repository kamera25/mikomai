# Mikomai for macOS

SwiftUIデスクトップアプリはRustの `mikomai-core` / `mikomai-adapters` をC FFI経由で利用します。SwiftチャットはローカルGGUFによるWorker回答と、LLM planner・device tools・複数stepのportable AgentLoopを使用します。CoreのPlannerには会話履歴、添付テキスト・画像解析結果、検索資料を渡し、Agentの内部イベントや生の機器出力は通常の回答として表示しません。

CoreがLLMの判断と回答生成を管理し、モデル読み込み・実推論は `mikomai-adapters::local_llama` に分離しています。FFIはその入口とSwift callbackの変換を担当します。

設定でVisionを有効にし、画像対応Gemma 4 GGUFと対応するmmprojを指定すると、PNG/JPEGをチャットに添付できます。画像は4ファイルまで、各8 MiB・合計16 MiB・各16,777,216画素以下です。画像の解析結果を非信頼の参考資料としてCoreへ渡します。テキストのみのモデルや不正な画像はエラーになります。

## Agentとネットワーク操作

AgentはFFI allow-list経由で疎通確認、route/IP情報、serial port列挙、`get_state`、`fetch_config`、`fetch_routing`、`fetch_arp`、`network_show`、packet解析/準備/安全確認、NW図、Cisco設定validate/convert、SurrealDB graph/MAC lookup、ベクトルRAG検索を利用します。機器調査はSwift callbackがmacOS Keychainから資格情報を取り出し、Netmiko runnerを呼び出します。秘密情報はplanner schemaやLLM promptへ渡さず、取得した設定出力もredactしてから共有します。

設定変更、serial console送信、FTP/TFTP file transferはhash付きOperationPlanを作成し、Swiftの確認・承認UIに送ります。実行は承認後だけ可能で、FFIのplan hash gate、開始済み状態確認、結果確定を通ります。資格情報はSwift Keychainから必要なFTP接続に限って渡します。Ask-user choiceは候補を表示し、返信を同じpending Agent taskへ続けます。NW図生成はSVG artifactとして保存・表示します。

React/Tauri UI/runtimeとIPC、専用MCP server process群を削除しました。旧Scheduler/Watch/TaskAuditの画面レイアウトは再現していませんが、定期実行、CPU監視、通知、run historyは「CPU監視」に、agent task履歴・再開は左アイコンから開く独立した「エージェント履歴」に分けています。操作監査は会話履歴のメニューから表示できます。GUIの専用ネットワークツール（TCP接続テスト、Ping/Trace、ARPテーブル、ルーティング）と機器一覧の診断ボタンは廃止し、ネットワーク調査はチャットのAgent経由で実行します。旧 `watches.json` とtask event JSONはSwift版Application Supportへ一度だけ取り込みます。agentの登録済みtool契約はportable FFI/Swift callback/承認済みadapterへ移しています。古いtool名と現在の実行経路は [`doc/tauri-retirement.md`](../doc/tauri-retirement.md) に記録しています。

## 設定と起動

設定は `~/Library/Application Support/MikomaiDesktopMac/settings.json` に保存します。既存インストールのsettingsは初回読込時に一度だけimportし、その後はSwift-native保存先を使います。接続情報はSwift側のローカル保存、パスワード類はmacOS Keychainに保存します。GGUFモデルは同梱せず、設定画面から選んで読み込みます。RAGには旧SurrealDB pathの `rag_chunk` を再利用し、Multilingual E5-Large modelを初回利用時にFastEmbed cacheへ取得します。

監視定義と履歴は `~/Library/Application Support/MikomaiDesktopMac/watches.json` に、Agent task event/evidenceは同じディレクトリの `agent-events/` に保存します。操作監査は `audit/operations.ndjson` に記録します。旧Tauri監視・task auditのファイルがある場合は初回起動時に不足分だけ取り込みます。

リポジトリから起動する場合は `./mikomai-desktop-mac/run.sh`、配布appを作る場合は `./mikomai-desktop-mac/build-app.sh` を実行します。Netmiko wrapperの共有Python資産とmacOS arm64 sidecarは `mikomai-core/assets/` 配下にあります。

CLIは同じFFI推論器を使います。`MIKOMAI_MODEL_PATH` を指定するか、Swift-native/既存設定にGGUF pathを設定して、`npm run cli -- chat "F220のVLAN設定方法を教えて"` のように実行します。モデルが見つからない時は非生成ナレッジ応答を行うため、LLM回答を検証する場合はモデルを設定してください。

CLIのSurrealDB/RAGインデックスは `~/Library/Application Support/MikomaiCLI/surrealdb` に保存します。Swift版の既存データベースとは分け、CLI検証中もAgentを起動できるようにしています。`MIKOMAI_GRAPH_DB_PATH` による明示的な指定は優先されますが、別プロセスで使用中のデータベースは同時に開けません。Swift版を再度起動すると、既に起動している同じアプリを前面に表示します。

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

### NW図の表示

「LAN 192.168.1.0/24にrouter01とswitch01を接続したNW図を作成して」のように構成を指定すると、共有CoreのPlotterが会話履歴・添付情報を参照してnwdiag DSLを生成し、検証後にSwift callbackがSVGを描画します。DSLを直接コードブロックで指定する場合はモデル推論を省略します。生成した図はチャット内で表示し、「拡大」「SVGを保存」を利用できます。SVGは回答に埋め込むため、履歴を開き直しても表示できます。構成不足や描画失敗では確認を求め、再生成は上限を設けています。

描画にはnwdiag・Pillow・SVG用依存を導入したPythonが必要です。既存の`venv/bin/python`、または`MIKOMAI_PYTHON`で指定した環境を使います。SVGの自動保存先はApplication Support内の`MikomaiDesktopMac/artifacts`で、検証時には`MIKOMAI_ARTIFACTS_DIR`で変更できます。

表示経路の検証は、Core/Swiftテストに加えて以下で実施できます。macOSのWebKit描画プロセスへのアクセスが必要です。`MIKOMAI_DIAGRAM_CHECK_MODEL`にGGUFパスを指定すると自然文からの生成も検証します。

```sh
MIKOMAI_WINDOW_CHECK_SOURCE="$PWD/mikomai-desktop-mac/Tests/NetworkDiagramChecks/NetworkDiagramChecks.swift" sh mikomai-desktop-mac/test-chat-window.sh
```

### アクセシビリティとキーボード操作

アイコンボタン、履歴、ワークスペースタブ、機器・監視の操作にはVoiceOver用の名前を付け、選択行には選択状態を公開します。NW図の代替テキストにはMarkdown画像のタイトルを使用します。

アプリ内のボタン・選択メニュー・スイッチ・入力欄はTabで次へ、Shift+Tabで前へ移動できます。システム全体の「キーボードナビゲーション」を有効にしなくても操作できます。Enter／Spaceでフォーカス中のボタンを実行し、スライダーは矢印キーで値を調整します。ホスト編集のEnterはフォーカス中の操作を優先し、入力欄では保存を実行します。日本語変換中は保存しません。Escでキャンセルできます。

履歴行は完全な会話名と選択状態を読み上げ要素に直接設定し、Enterで選択、F2で名前変更、VoiceOverのカスタムアクションで名前変更・削除ができます。質問欄の機器名候補が開いている場合だけ、Tabで候補を補完します。Shift+Tabは候補表示中も前へ移動します。日本語変換中は入力メソッドを優先します。

`sh mikomai-desktop-mac/test-accessibility.sh` は全ワークスペースと設定の全カテゴリの操作名、履歴の選択・名前変更、ホスト編集の前後移動、フォーカス中の操作と保存の優先順、独立した機器一覧の編集ボタン、無効状態、スイッチ・スライダーのキー／アクセシビリティ操作を検証します。`test-chat-composer.sh` は入力保持、Tab移動、候補補完、IME、設定欄の改行を検証します。VoiceOverの実音声は自動試験に含みません。

チャットのユーザー・AIの発言全文、エージェントの状態・目的・次のアクション・詳細・展開した実行内容、ホスト一覧の各セルもTab／Shift+Tabの対象です。フォーカス枠を表示し、画面外の項目へ移動するとスクロールします。ホストのセルには機器名・列名・値を付け、空欄は「未設定」と読みます。資格情報は設定の有無のみ公開します。

機器の詳細登録・編集画面では、入力済みでも項目名と必須・入力条件の説明が残ります。パスワード欄はネイティブの保護された入力欄です。入力エラーと資格情報の保存説明もTabで確認でき、通常の入力欄はTab直後の文字が次の欄に入るよう即座にフォーカスを切り替えます。
