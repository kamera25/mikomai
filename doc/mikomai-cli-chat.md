# mikomai-cli chat

`chat` はヘッドレスCLIから質問に回答するコマンドです。`MIKOMAI_MODEL_PATH` または現在のDBの `settings:current` から有効なGGUFモデルを取得できる場合、`mikomai-app` を直接呼び出し、Swift版のFFIと同じローカルLLM・資料検索の実装を使います。モデル未設定時は `mikomai-core` のアプリケーションサービスとローカルMarkdown用知識ストアへフォールバックします。会話履歴、Keychain、実機接続状態は共有しません。

CLIの既定DBは `~/Library/Application Support/MikomaiCLI/surrealdb`、macOSアプリは `~/Library/Application Support/MikomaiDesktopMac/surrealdb` です。`MIKOMAI_GRAPH_DB_PATH` で明示したDBを使うこともできますが、組込みRocksDBは複数プロセスで同時に開けません。GUI/CLIの単一正本と同時起動の両立は未実装です。CLIでGUIと同じモデルを選ぶには `MIKOMAI_MODEL_PATH` を指定してください。旧 `settings.json` と `MIKOMAI_SETTINGS_PATH` の自動読込みは廃止しました。

```bash
npm run cli -- chat "FITELnet F220 の VLAN 設定方法を教えて"
npm run cli -- --json chat "ルーティングの基本を説明して"
npm run cli -- chat "FITELnet F220 の VLAN 設定方法を教えて" --debug
```

モデルのテンソル読み込み・モデル情報・読み込み進捗などのログは、`--debug` または `-d` を指定した場合だけ標準エラー出力に表示します。モデル読み込み時の `control-looking token`、`special_eog_ids contains '<|tool_response>'`、`using full-size SWA cache` の通知もデバッグ時だけ表示します。それ以外の警告やエラーは通常実行でも表示します。

`--debug-jsonl` は、Swift版のデバッグ記録と同じ `timestamp`・`kind`・`payload` 形式で、1行につき1レコードを標準出力へ出します。npmのバナーを混ぜないよう、保存時は `--silent` を指定してください。

```bash
npm run --silent cli -- chat "F220のVLAN設定方法を教えて" --debug-jsonl > mikomai-debug.jsonl
# 標準エラー出力の詳細ログも取得する場合
npm run --silent cli -- chat "F220のVLAN設定方法を教えて" --debug-jsonl --debug > mikomai-debug.jsonl 2> mikomai-debug.log
```

入力は `cli_request`、ストリームは `core_stream`、最終応答は `core_response`（`payload.status` が成功時 `0`、失敗時 `1`、回答またはエラーは `payload.text`）に入ります。ローカルモデル使用時は、SwiftのFFIも利用する共通の `mikomai-app` から `llm_request`・`llm_response` などの内部記録も受け取り、そのまま出力します。モデル未設定時はMarkdown検索へフォールバックし、処理後に `agent_event` を出力します。`cli_request.payload.backend` で `local_model` / `markdown` を判別できます。

このオプションでは通常の回答テキストを別途標準出力へ追加しません。`--json` / `-j` との同時指定、および `chat` 以外での指定はエラーになります。Rustから返された実行時エラーもJSONLへ記録し、標準エラー出力と終了コード `1` で通知します。引数エラーやネイティブライブラリの強制終了では最終レコードが出ない場合があります。記録には入力、プロンプト、資料、回答が含まれます。

`nw-docs/` が存在する場合、実行時に資料を取り込み、検索結果を回答に利用します。保存先は `MIKOMAI_KNOWLEDGE_DIR` で変更できます。

JSON出力では回答は `data.response` に入ります。

```json
{"ok":true,"data":{"response":"..."}}
```

CLIとmacOSアプリは別々の実行経路です。CLIでの動作確認は、Swift UI、アプリ内LLM設定、実機MCP操作の検証にはなりません。

### 自機のルーティング照会

`chat "localhost のデフォルトルートはどこ？" --debug-jsonl` は自機のデフォルト経路を取得し、日本語の説明と取得した経路情報を応答として返します。`chat "自機のルーティングを確認して" --debug-jsonl` はルーティングテーブル全体を取得します。macOSではそれぞれ `route -n get default` と `netstat -rn`、Linuxでは `ip route show default` と `ip route show table all` を使用します。LLMや資料検索は不要です。取得失敗や空の結果を一般論で補完せずエラーとして返します。デスクトップもcoreの同じ判定と `self_network_route` の `scope` (`default` / `table`) を使用します。

自機の経路照会への応答はcoreの共通フォーマッターで日本語化します。デフォルト経路の出力にgateway/interfaceが明示されている場合は、「デフォルトゲートウェイ」「使用インターフェース」として表示します。元の経路情報も残し、未取得の値は推測しません。

### サービス名によるポート確認

`chat "127.0.0.1 のsshを確認して" --debug-jsonl` のように、ポート番号の代わりに `ssh`、`dns`、`https` などを指定できます。`tcp/22`・`22/tcp`・`dns/tcp` も解決します。core の定型入力と Agent の `port`（文字列）、`query`、`service` パラメーターで同じ解決処理を使います。

定義は `mikomai-core/src/service_ports.json`、解決処理は `service_ports.rs` に分離しています。定義には `name`、`port`、既定の `protocol`、利用可能な `transports`、任意の `aliases` を指定します。組み込み定義の編集は再ビルド後に反映されます。連携側では `ServiceRegistry::from_json` で独自定義を読み込めます。重複する名前・別名や不正な定義は拒否されます。

疎通確認の実行はTCP専用です。`dns` の既定値はTCP/53で、DNS問い合わせの応答確認を行うものではありません。`udp/22`・`dns/udp` はUDPとして解決されますが、TCP接続チェックでは実行しません。`ntp`・`snmp` の既定値もUDPのままです。未知のサービスや矛盾する通信方式を推測して実行しません。
