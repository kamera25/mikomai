# ARP の透過的な Graph / Canonical 化

Agent は `get_state` の `resource: "arp"` を呼び、検証済みの UniversalArpTable JSON を受け取ります。ベンダーの生出力や解析用プロンプトは Agent の evidence に返しません。`fetch_arp` と ARP の `network_show` も同じ処理を通ります。

処理の順序は core の `network::arp_state::get_state` にあります。

1. 対象機器の最新 ARP observation を Graph で検索します。鮮度は既存の20分基準です。新しい失敗した収集があるとき、古い成功データを復活させません。
2. 有効な Canonical データがあればそのまま返します。MAC が見つからない場合も、そのテーブル全体を根拠に判断します。
3. 新鮮な raw observation だけがあれば再取得せずに Canonical 化します。未収集・期限切れなら登録済み機器の読み取り用 callback で取得します。
4. raw を Graph に保存し、アドレスとインターフェースの候補を列位置に依存せず抽出します。LLM は候補インデックスと列挙された属性だけを選択します。
5. llama.cpp の GBNF sampler が JSON構造、インデックス範囲、属性の列挙値を生成時に制約します。その後 core がスキーマ、欠落・重複、同一原文行での共起、age の根拠を検証します。失敗時は最大4回まで修正を要求します。
6. Canonical と候補・原文行の evidence を Graph に保存します。元の取得時刻を保持し、Canonical 化によって鮮度を延長しません。同じ収集の保存は冪等です。IP の正規化済み facts も保存します。

ベンダーごとのコマンド選択は収集側の責務ですが、Canonical 化の解析は共通です。生出力が空、取得失敗、推論失敗、検証失敗の場合は「存在しない」と判定しません。大きすぎる原文を切り捨てて部分的なテーブルを確定することもありません。

Canonical 化専用の推論は Grammar を実行できるロード済み GGUF が必要です。AFM の自由文生成はこの制約を保証できないため、この内部処理には使いません。ロード済み GGUF がなければ明示的なエラーになります。今後ほかの制約付き推論器を接続する場合も core の contract と検証を共用できます。

Standalone CLI の localhost ARP も同じ Graph adapter と core 処理を通ります。`MIKOMAI_GRAPH_DB_PATH` で検証用DBを選べます。診断用の Canonical 化推論ログは `arp_canonicalization_request` / `arp_canonicalization_response` として計画LLMのログから区別されます。

## 検証

- core: `cargo test -p mikomai-core network::`
- Graph read-through: `cargo test -p mikomai-adapters arp_state`
- FFI Agent: `cargo test -p mikomai-ffi arp_`
- 実LLM: `MIKOMAI_ARP_TEST_MODEL=/path/to/model.gguf cargo test -p mikomai-adapters real_llm_canonicalization_and_graph_cache -- --ignored --nocapture`
- 必須CLI: `npm run --silent cli -- chat "localhost のARPテーブルに44:55:66:b3:37:22は存在する？" --debug-jsonl`

実LLM試験は合成した Yamaha・Cisco・未知の列順を使います。実機とのSSH接続を検証したものではありません。`MIKOMAI_ARP_TEST_DB` を指定すると試験後もGraphを残し、同じDBに対するCLIでキャッシュヒットと最終回答を確認できます。
