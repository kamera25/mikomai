# mikomai-app 段階1の実装と検証

2026-10-06 に greenfield_redesign.md §9 の段階1を実施。

## 実装

- workspace に `crates/mikomai-app` を追加。
- 承認計画・実行済みclaim・Agent保留タスク・RAG取込状態・graph・監査・Watch・backend選択を `MikomaiService` のフィールドへ移動。
- portable runtime と operation worker runtime をサービス所有の1つのTokio runtimeへ統合。
- FFI は既存C ABIの委譲とCLI向けRust関数の再公開だけを担当。static宣言は0個。
- C ABIの39関数は引数・戻り値・unsafe区分とdylibのエクスポートを維持。
- 既存テストと画像fixtureをappへ移動し、FFI/appの承認状態共有とresult解放を確認する境界テストを追加。

段階2のCLI依存変更、Command/Event/Query API、worker管理、Scheduler等は今回の範囲外。既存の保存形式とcallback契約は維持した。サービスの独立インスタンス向けAPIは後続段階で実装し、旧ABIはapp内の共有サービスを使う。

## 受入条件と結果

事前の条件は、FFIのstaticが0個、ABI互換、共有状態と承認・二重実行拒否の維持、Runtime統合後のWatchと非同期FTP/TFTP実行、CLIの最終status 0と資料に沿うF220回答。

- **合格**: `cargo build -p mikomai-ffi`（終了0）。ソース署名比較と `nm -gU target/debug/libmikomai_ffi.dylib` で39関数を確認。
- **合格**: `cargo test -p mikomai-app -p mikomai-ffi -p mikomai-cli -- --test-threads=1 --skip c_abi_reports_errors_answers`（終了0）。app 48件、CLI 6件、FFI境界1件が通過。実モデルを要する5件はignore。
- **既存不合格**: `c_abi_reports_errors_answers_from_local_documents_and_frees_results` はfixtureの `NATIVE-FFI-ANSWER-7319` を回答に含めず失敗。HEADの変更前FFIソースと変更前Cargo.lockを `/tmp/mikomai-stage1-baseline` に置き、資料includeパスだけ元の場所へ補正して同じ失敗を確認。テスト本体と期待値は変更していない。
- **合格**: `MIKOMAI_GRAPH_DB_PATH=/tmp/mikomai-stage1-cli-graph npm run --silent cli -- chat "F220のVLAN設定方法を教えて" --debug-jsonl`（終了0）。実際の設定済みGGUFとE5 RAGを使用。stdout全438行をJSONとして解析し、timestamp/kind/payload、入力、reference_context、llm_request/response、最後のcore_response.status 0を確認。Access/Trunk VLANのGigaEthernet・vlan-id・channel-groupと出典を `nw-docs/fitelnet/02-{1,2}_make_*_vlan.md` に照合。質問は手順なのでテンプレートのプレースホルダは期待通り。
- **未検証**: Swift UI操作、実機への接続・変更、Windowsビルド。FTP/TFTPの確認はloopbackであり実機成功とは扱わない。

最初のsandbox内CLIは既定DBへの書込みが拒否され、RAGなしの不適切な回答になったため不合格と判定。上記の一時DBを用いる実行で資料検索と回答内容を改めて確認した。ソケットを使うRustテストはsandbox外で実施。

ログ（今回の実行環境の一時ファイル）:

- `/tmp/mikomai-stage1-build.log`
- `/tmp/mikomai-stage1-tests.log`
- `/tmp/mikomai-stage1-baseline.log`
- `/tmp/mikomai-stage1-vlan-isolated.jsonl`
- `/tmp/mikomai-stage1-vlan-isolated.stderr`
