# GraphDBルータスキーマ検証記録

対象: 機能別のネイティブGraphDBスキーマ31種類。OpenConfigは参照資料として使用し、`openconfig`テーブル/ノード種別/relationは作成しない。

## 受け入れ条件

- 不足テーブルがSurrealDBで実際に定義され、機能フィールドを直接保持する。
- 型・必須キー・値域・複合一意キー・不正入力の拒否が働く。
- 装置およびVRFの違いでノードが上書きされず、再取り込みが重複を作らない。
- 各機能の装置エッジ、個別relationによるサブグラフ、機能factsを取得できる。
- 古い観測による上書きを防止し、機能の鮮度を別機能の新しい観測で誤判定しない。
- DB再オープンとスキーマ再定義で既存データを保持する。
- 最終コードで必須の `chat --debug-jsonl` を実行し、全行のJSON解析、内部処理、最終status/textを確認する。

## 実DB・関連テスト: 合格

```bash
RUST_BACKTRACE=1 cargo test -p mikomai-adapters portable_graph::tests --lib -- --nocapture --test-threads=1
cargo test -p mikomai-core planner::tests --lib
python3 scripts/generate-router-schema.py --check
git -c core.fsmonitor=false diff --check
```

終了コードはすべて0。GraphDB試験3件、planner試験8件が合格。

実DB試験は一時ディレクトリのRocksDB/SurrealDBを使用。全31テーブルの列・構造化データ、最大uint64 SPIの文字列保存、装置/VRF分離、重複抑止、OSPF/LLDPの個別relation、型・値域・未知フィールド・重複入力・SQL注入形式のテーブル指定の拒否、古い観測の上書き防止、TTL判定、DB再オープン後の保持を確認した。`INFO FOR DB`で `ospf` と `lldp` の存在、`openconfig` テーブルの不在も確認。機能名がplannerのrelations・GraphDataKind・SubgraphRelationと一致することを試験している。

ログ: `/tmp/mikomai-router-schema-tests.log`, `/tmp/mikomai-router-schema-planner-tests.log`。

参照YANG全31エントリのSHA-256をDownloadsの実ファイルと再照合し一致を確認した。

## CLI JSONL

GGUFモデルは既存設定から読み込んだ `local_model` 経路。ドキュメントは既存の `nw-docs`、GraphDBの設定先は以下の一時パスに分離した。

```bash
MIKOMAI_GRAPH_DB_PATH=/tmp/mikomai-router-schema-cli-db npm run --silent cli -- chat "F220のVLAN設定方法を教えて" --debug-jsonl
MIKOMAI_GRAPH_DB_PATH=/tmp/mikomai-router-schema-cli-feature-db npm run --silent cli -- chat "LLDPとOSPFはルータでそれぞれどのような役割を持つのか、違いを説明してください" --debug-jsonl
```

基本質問: 最終コードで再実行し終了コード0、438行をすべてJSON解析。入力、local_model backend、llm_request/llm_response、reference_context、最後のcore_responseを確認。最終 `payload.status == 0`、空でない `payload.text`。F220のAccess/Trunk VLANテンプレートと引用先を `nw-docs/fitelnet/02-2_make_access_vlan.md` / `02-1_make_trunk_vlan.md` に照合し一致。回答は設定テンプレートとして提示され、内部の推論指示の露出なし。正常応答・形式・この質問の資料整合性は合格。

変更関連質問: 終了コード0、360行をすべてJSON解析。`cli_request`, `llm_request`, `llm_response`, `reference_context`, `reference_context_compaction`, `core_stream`, 最後の `core_response` を確認。最終 `payload.status == 0`、空でない `payload.text`。LLDPがL2で隣接情報を交換し、OSPFがL3で経路を計算するという区別を確認した。

LLDP/OSPF回答は出典なしの一般説明であり、実機の状態や新規GraphDBテーブルの保存経路を通った証拠はない。この質問は正常応答/形式の確認で、資料根拠や新規スキーマ自体の合格判定には使用していない。新規スキーマの判定は上記の実DB試験による。

ログ: `/tmp/mikomai-router-schema-cli-basic.jsonl`, `/tmp/mikomai-router-schema-cli-basic.stderr`, `/tmp/mikomai-router-schema-cli-feature.jsonl`, `/tmp/mikomai-router-schema-cli-feature.stderr`。

## 未検証・今回の対象外

稼働中アプリのDBへの即時適用、Swift UIでの表示、実機とのgNMI/SSH通信、新規全機能の自動収集・正規化・設定投入は未検証。スキーマは更新後のGraphDB初期化で適用される。
