# Router Canonical化の検証記録

2026-10-06。仕様・制限は [router-canonicalization.md](router-canonicalization.md) を参照。

## 関連テスト: 合格

- `cargo test -p mikomai-adapters router_ --lib`: 6件合格、実GGUF専用の1件は通常実行ではignored。
- `cargo test -p mikomai-adapters portable_device --lib`: 7件合格。
- `cargo test -p mikomai-core planner --lib`: 13件合格。
- `cargo test -p mikomai-ffi executor_ --lib`: 3件合格（新規router、既存ARP、既存interfaces）。
- `python3 scripts/generate-router-schema.py --check`: 終了コード0。
- `git -c core.fsmonitor=false diff --check`: 終了コード0。

全31テーブルの必須/任意フィールド、全フィールド型、型/値域/複合キー、uint64 SPI保持を合成JSONから再構成した。候補範囲外、型違い、必須キー欠落、原文範囲外、重複、根拠のない空配列を拒否し、修正再試行の回数を確認した。Canonical envelopeの余分なフィールドと装置/時刻の不一致も拒否する。

一時RocksDB/SurrealDBの試験でraw保存→推論失敗→raw再利用→Canonical/facts保存→取得・推論なしの再利用、期限切れ/refresh、取得失敗後の旧Canonical再利用禁止、収集時刻保持を確認した。Graphの既存31テーブル実DB試験も合格。

FFIのfake Swift callbackで登録IDからhostnameへの解決、`get_state`のresource/対象、CanonicalだけのAgent返却、facts保存、キャッシュ、refresh失敗を確認。実機通信とは区別する。NDPはlocalhostと登録済みリモートを許可し、未登録リモート/localhost指定混同を拒否する。

全31テーブルがplannerのresource enumとSwiftの取得コマンド選択肢にあることも照合した。

ログ: `/tmp/mikomai-router-canonical-tests.log`, `/tmp/mikomai-router-device-tests.log`, `/tmp/mikomai-router-planner-tests.log`, `/tmp/mikomai-router-ffi-tests.log`。

## Swift: 構文合格、アプリ全体ビルドは環境エラー

`swiftc -frontend -parse mikomai-desktop-mac/Sources/MikomaiDesktopMac/DesktopModel+Tools.swift` は終了コード0。

`swift build` は最初、ユーザーキャッシュの書き込み制限で失敗。cacheを `/tmp` に切り替えて再実行したが、既存UIの `SwiftUIMacros.StateMacro` pluginがツールチェーンに存在せず失敗した。変更した取得コマンドのSwift型検査/リンク完了を合格とは扱わない。

ログ: `/tmp/mikomai-router-swift-build.log`, `/tmp/mikomai-router-swift-parse.log`。

## 実GGUF試験

既存settingsのmodelPathを `MIKOMAI_ROUTER_TEST_MODEL` に渡し、`real_llm_router_selection_grammar` を `--ignored --nocapture` で実行した。入力は合成したDNS resolverとLLDP detailで、実ベンダー出力ではない。

最初の実行はDNS出力を `complete=false` と判定して拒否し失敗。任意フィールドの欠落が不完全の理由にならないことを推論契約に明記し、同じ入力・同じ期待normalizedで再検証した。初回失敗ログ: `/tmp/mikomai-router-real-llm-tests.log`。再検証ログ: `/tmp/mikomai-router-real-llm-tests-retry.log`。

再試験で、候補全体の番号を部分集合の番号と混同した選択も確認。候補に明示的なindexを追加し、検証エラーに選択値と原文行を含めて修正した。`/tmp/mikomai-router-real-llm-tests-indexed.log` では同じDNS入力が成功した。LLDPの複数行入力では根拠行範囲の誤選択と無根拠な空配列が検証で拒否され、精度調整が必要と分かった。

## CLI JSONL

通常DBでの初回実行はRAG書き込みがsandbox制限で失敗したため、一時GraphDBで基本質問と関連質問を再実行した。初回のstatus=0だけを内容合格にはしていない。

一時DBの基本質問「F220のVLAN設定方法を教えて」は終了コード0、438行すべてをJSON解析し、cli_request、reference_context、llm_request/response、最後のcore_response.status=0、空でないtextを確認。Access/Trunkのテンプレートと出典を `nw-docs/fitelnet/02-2_make_access_vlan.md` / `02-1_make_trunk_vlan.md` と照合した。

関連質問「LLDPとOSPFはルータでそれぞれどのような役割を持つのか、違いを説明してください」は終了コード0、360行すべてをJSON解析。LLDPはL2隣接発見、OSPFはL3経路計算という区別を確認した。出典なしの一般説明であり、Canonical化経路を実行した証拠はないため、このCLIだけで新規機能を合格とは扱わない。

これらは途中の確認ログであり、最終本番コードのCLI再実行結果を下に記録する。ログ: `/tmp/mikomai-router-cli-basic-final.jsonl` / `.stderr`, `/tmp/mikomai-router-cli-feature-final.jsonl` / `.stderr`。

## 未検証

全ベンダーの実出力精度、全31機能の実機SSH/serial、GUI表示、AFM用structured generation契約、生CLIからのネスト構造組み立て、単位変換/hex SPI変換は未検証または未実装。これらを合成データ試験やCLIのstatus=0で合格としていない。

## 最終本番コードのCLI再確認: 合格

次を最終本番コードで再実行し、両方とも終了コード0。基本質問は438行、関連質問は360行をすべてJSON解析した。入力・local_model・reference_context・llm_request/response・最後のcore_response.status=0・空でないtextを確認した。基本質問のAccess/Trunkテンプレートと資料引用、関連質問のL2/L3の区別を確認。一時DBでRAGの書き込みエラーは発生しなかった。新規Canonical化の判定はCLI一般回答とは区別する。

```bash
MIKOMAI_GRAPH_DB_PATH=/tmp/mikomai-router-canonical-cli-basic-db npm run --silent cli -- chat "F220のVLAN設定方法を教えて" --debug-jsonl
MIKOMAI_GRAPH_DB_PATH=/tmp/mikomai-router-canonical-cli-feature-db npm run --silent cli -- chat "LLDPとOSPFはルータでそれぞれどのような役割を持つのか、違いを説明してください" --debug-jsonl
```

ログ: `/tmp/mikomai-router-cli-basic-verified.jsonl` / `.stderr`, `/tmp/mikomai-router-cli-feature-verified.jsonl` / `.stderr`。

実GGUFの最終追加試験は **一部合格/全体不合格**。DNSは期待normalizedに一致し、GBNFによる候補インデックス制約が機能した。LLDPは4回の選択後もcomplete=falseを返し、Canonical化が拒否されたためテスト終了コード101（失敗時にRust panic runtimeのSIGABRTも記録）。LLDPの実モデル精度は未達として残す。ベンダー実出力での精度調整を後続で行う前提の叩き台であり、全31機能の実モデル成功とは扱わない。ログ: `/tmp/mikomai-router-real-llm-tests-indexed.log`。
