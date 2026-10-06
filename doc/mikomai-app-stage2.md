# mikomai-app 段階2の実装と検証

2026-10-06 に greenfield_redesign.md §9 の段階2を実施。

## 実装

`mikomai-cli` の依存を `mikomai-ffi` から `mikomai-app` へ切り替えた。
モデル読込み、通常/ストリーミングチャット、自機ARP、TCP接続確認の6箇所をappへの直接呼出しに変更。Cargo.lockとCLIマニュアルを更新した。

`cargo tree -p mikomai-cli --prefix none` で直接・推移依存ともFFIが含まれないことを確認。CLI内の `mikomai_ffi` 参照も0件。JSONL形式と既存のルーティング・回答生成を維持した。

## 事前の受入条件

- CLIがappに直接依存し、FFIへ依存しない。
- 既存CLIテストが通る。
- FastRouterのTCP照会がappのTCP実装を通り、対象・port・実測結果と回答が一致する。
- FastRouterに一致しない既存Agentモードの経路照会で、tool_call・observation・最終回答を確認できる。
- 必須F220質問で実モデルとRAGがappの共通処理を通り、資料に整合する回答を返す。
- 各JSONLの全行を解析し、最終core_responseのstatus 0と空でないtext、およびCLI自身の終了0を確認する。

## 結果

全項目 **合格**。

- `cargo test -p mikomai-cli`: 6件合格、終了0。出力形式とDB分離の既存テストを実施。
- `npm run --silent cli -- chat "tnc 127.0.0.1 -port 51502" --debug-jsonl`: 5行、終了0。検証プロセスが一時的に開いたloopbackソケットへ接続。`mode=fast_router`、`tool=self_network_test_connection`、host/port、成功したobservationを確認。TCP実装はappへ直接呼出し。
- `npm run --silent cli -- chat "localhost の127.0.0.1 のネクストホップはどこですか？" --debug-jsonl`: 5行、終了0。`mode=agent`、`tool=self_network_route`、`destination=127.0.0.1`、観測と回答の `lo0` を確認。`fast_route` レコードは出ていない。
- `MIKOMAI_GRAPH_DB_PATH=/tmp/mikomai-stage2-cli-graph npm run --silent cli -- chat "F220のVLAN設定方法を教えて" --debug-jsonl`: 438行、終了0。設定済みローカルGGUF、E5 RAG、作業用一時DBを使用。`reference_context`、`llm_request`、`llm_response` を確認。回答のGigaEthernet・vlan-id・channel-groupとAccess/Trunk VLANの出典を元のF220資料に照合。手順を尋ねる質問のため、テンプレートのプレースホルダは期待通り。

Agentモードの確認は既存の決定的な経路照会ショートカットであり、LLMによるAgentLoopの任意ツール選択を新たにCLIへ接続したものではない。SSH/Telnet/serialのワーカー経路、Swift UI、実機の変更は今回未検証。段階1で確認した既存の資料検索fixtureテスト不合格は今回の変更対象外。

ログ:

- `/tmp/mikomai-stage2-tests.log`
- `/tmp/mikomai-stage2-dependencies.txt`
- `/tmp/mikomai-stage2-fast.jsonl` / `/tmp/mikomai-stage2-fast.stderr` / `/tmp/mikomai-stage2-fast.exit`
- `/tmp/mikomai-stage2-agent.jsonl` / `/tmp/mikomai-stage2-agent.stderr` / `/tmp/mikomai-stage2-agent.exit`
- `/tmp/mikomai-stage2-vlan.jsonl` / `/tmp/mikomai-stage2-vlan.stderr`
