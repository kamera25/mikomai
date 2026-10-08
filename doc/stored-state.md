# query_state / diff_state

保存済みCanonical StateをCoreで決定的に検索・比較するread-onlyプリミティブ。
内部で実機接続、収集、LLM推論、自然言語解釈は行わない。新しい観測が必要な場合だけ
既存のget_stateを呼び、LLMにはquery_stateで必要な部分集合を渡す。
get_stateの既存入出力は変更しない。

## 既存基盤と責務

- CoreのInterfaceは `version: "1.0"`, `metadata`, `interfaces`、ARPは
  `version: "1.0"`, `metadata`, `arp_table`。Interfaceのリンク状態は `status`
  (`up/down/unknown`)であり、admin/operフィールドは存在しない。
- Router Canonicalは `version`, `metadata`, リソース名の配列。
  `crates/mikomai-adapters/src/schema/router-resources.json` がフィールド・複合identityを定義。
- get_stateはAdapterのGraph read-throughを使い、キャッシュミス時に収集・正規化する。
  Canonical化の推論は既存Adapterの責務。query/diffには推論を組み込まない。
- SurrealDBの `observation` は機器、種別、source、取得時刻から作るIDで履歴を保存する。
  全ネットワークの原子的・不変なSnapshot履歴は存在しない。
  今回は観測1件をリソース単位のSnapshotとして参照する。DBや依存ライブラリは追加しない。
- Core `network::state` は入力/出力型・スキーマ・純粋演算、Adapter `state` は既存カタログの
  再利用と保存参照、PortableGraph/store brokerはDBアクセス、Appは実行と接続入口、
  CLIは引数と出力、LLMは公開契約に従った呼出しを担当する。
  CLI chatでも保存済み状態の操作依頼は共通Agentへ渡す（モデル設定が必要）。説明依頼はWorkerに残す。
  Resolver/transport/資格情報取得はquery/diff経路で使用しない。
- エラーは既存の `Result<_, String>`、Agent実行結果は既存の `ToolResult` を使用する。

## Snapshot指定

`snapshot_id` / `before` / `after` は観測IDまたは `latest`。
`device` は保存時のCanonical機器名（登録ID/IPを自動変換しない）、`resource` は
`interfaces`, `arp` またはRouterカタログのtable名。
`scope` は既定 `all`。Interfaceでは取得時の `get_state.parameters.interface` と同じ文字列を指定する。
例：LAN1だけ取得した観測は `scope: "LAN1"`。全Interface観測と混ぜない。
他のリソースは `scope: "all"` のみ。

latestは取得時刻の降順、同時刻はID昇順で1件に解決する。TTLで再取得したり、
失敗した最新観測を飛ばして過去の成功を返したりしない。同じselector同士のdiffは一度だけ解決する。
明示IDもdevice/resource/scopeに一致する必要がある。
返却 `snapshot` / `before` / `after` メタデータには実際のID、取得時刻、機器、リソース、scope、
source_id、normalizer_version、model_version、completeが含まれる。
取得時刻から鮮度を判断する。保存参照は現在の実機状態の保証にはならない。

## query_state

CLI:

```sh
npm run --silent cli -- query-state '{"snapshot_id":"latest","device":"R1","resource":"interfaces","scope":"eth1","filter":{"name":"eth1"},"fields":["name","status"],"limit":10}'
```

入力は上記JSON。`snapshot_id/device/resource` が必須。`filter/fields/scope/limit` は任意。
filterは既存フィールドに対するscalar（文字列/数値/bool/null）の完全一致AND。
オブジェクト、配列、演算子、DSLは受け付けない。`null` は明示nullだけに一致し、欠落に一致しない。
fields未指定または空配列は全フィールド。それ以外は指定したトップレベルフィールドだけ返し、
欠落フィールドは欠落のまま維持する。未知フィールドと余計な入力キーはエラー。

出力例（ID/source/version等は実際の観測値）:

```json
{
  "snapshot": {
    "snapshot_id": "0123456789abcdef",
    "device": "R1", "resource": "interfaces", "scope": "eth1",
    "collected_at": "2026-10-08T00:00:00+00:00",
    "source_id": "get_state.interfaces:eth1",
    "normalizer_version": "interface-constrained-index-v1",
    "model_version": "1.0", "complete": true
  },
  "availability": "complete",
  "results": [{"name":"eth1","status":"down"}],
  "matched": 1, "truncated": false
}
```

条件不一致は `results: [], matched: 0, availability: "complete"`。
Canonical未取得/収集または正規化失敗の保存済み観測は `availability: "unavailable"`,
`complete: false`, `results: []`。部分的または完全性を証明できない旧Canonicalは `partial`。
Snapshot自体が存在しない場合は `State snapshot not found` エラー。
無効なCanonical構造・重複identityはエラー。未知/未取得を正常な空状態と解釈しない。

## diff_state

```sh
npm run --silent cli -- diff-state '{"before":"0123456789abcdef","after":"latest","device":"R1","resource":"interfaces","scope":"eth1","limit":100}'
```

`before/after/device/resource` が必須。scope/limitはqueryと共通。
返却 `changes` 例:

```json
[{"operation":"replace","path":"/interfaces/[\"eth1\"]/status","before":"up","after":"down"}]
```

完全な出力は `before` と `after` のSnapshotメタデータ、`changes`, `total_changes`, `truncated`。
変更なしは `changes: []`, `total_changes: 0`, `truncated: false`。
pathはJSON Pointerのエスケープ規則（`~0`, `~1`）を使った論理リソースパス。
配列indexではなくidentity値のJSON配列を1つのpath segmentとして使用する。
元のCanonical配列へのJSON Patchとして直接適用するパスではない。

- Interface identityはname、ARPは(ip_address, interface)、Routerは既存カタログのidentity。
  重複/missing identityは曖昧な比較を避けエラー。
- リソース配列の順序とInterfaceのipv4_addressesの順序は無視する。
  その他のネストした配列はスキーマに順序無視の明示規則がないため値として比較する。
- フィールド変更は構造的に辿り、add/remove/replaceを返す。
  addはafterのみ、removeはbeforeのみ、replaceは両方。明示nullはJSON nullとして返し、
  欠落側のbefore/afterキーは省略する。unknownなどの列挙値はそのまま比較する。
- 取得時刻などenvelopeメタデータは変更一覧の対象外で、別メタデータに残す。
  リソース内のカウンタ、age_seconds、時間値は除外せず観測値として比較する。
- 一方でも不完全、取得範囲違い、モデル/Normalizerバージョン違いはエラー。
  部分取得の欠落を削除扱いしない。未知バージョン間の自動migrationは行わない。
- identityのJSON表現の辞書順、その中でフィールド名の辞書順に処理するため出力は決定的。

## 制限と今後

件数limitは既定100、1〜1000。queryの一致件数とdiffの変更総数を返し、上限を超えた場合は
truncated=true。返却JSONは最大1 MiB。それ以上はフィールド/条件/件数を絞るエラー。
selectorは非空・最大256 bytes、filter文字列は最大4096 bytes。

既存観測は同じsource/取得時刻のCanonical化リトライで更新されるため、不変Snapshot契約ではない。
完全性は対応する検証済みNormalizerとCanonicalスキーマ検証から判定する。
旧portable-agent由来のデータは完全性を保証しない。観測に記録されない収集試行は履歴に現れない。
Interface/ARPの収集失敗も今後はraw-only観測を保存し、古い成功へ戻らないようにした。

NDP/旧routes/configのように統一されたCanonicalリソース・identity契約がないデータは対象外。
任意の2 JSONモデルを直接渡す公開API、グラフクエリDSL、ページング、全機器の原子的Snapshotは未実装。
今後は既存Graph関係を使う限定的なjoin、履歴一覧、cursor取得、明示的な取得完全性・失敗原因、
不変Snapshot化、スキーマversion移行を拡張できる。

## 検証

Core unit testsはフィルタ、projection、上限、null/欠落、不完全性、追加/削除/変更、
順序変化、identity曖昧性、バージョン不一致を確認する。
Adapter integration testsは一時DBのみを使い、実機/LLMなしで履歴ID、latest、scope、
不存在、失敗した最新観測を確認する。
入力/出力JSON Schemaは `network::state::input_schema/output_schema`、厳密入力と出力型は同モジュール。
