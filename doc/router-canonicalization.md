# 新規GraphDBリソースのCanonical化（叩き台）

2026-10-06。対象は `router-resources.json` の全31テーブル。ARPの「Graph確認 → raw取得/保存 → 候補抽出 → 制約付き推論 → 検証 → Canonical/facts保存」を参考に、スキーマ駆動の共通処理として実装した。ベンダー出力での精度調整はこれから行う。

## 対象

`ospf`, `ospf_neighbor`, `isis`, `bfd`, `lldp`, `ndp`, `vrrp`, `lacp`, `tunnel`, `routing_policy`, `prefix_set`, `policy_forwarding`, `acl_entry`, `acl_binding`, `nat`, `dhcp_relay`, `qos`, `qos_interface`, `pim`, `igmp`, `mpls`, `dns_server`, `syslog_server`, `aaa_server`, `snmp`, `telemetry_subscription`, `platform_component`, `system`, `mac_entry`, `ipsec_connection`, `ike_sa`。

各テーブルのフィールド型・必須キー・値域・列挙値を既存カタログから取得する。機能ごとの初期解釈ルールを `resource_hint` に置き、同じCanonical化エンジンから参照する。既存ARP、interfaces、routes、BGPの処理をこの共通実装に移行する変更ではない。

## 実装箇所

- `crates/mikomai-adapters/src/router_canonicalization.rs`: 候補抽出、機能別ヒント、GBNF生成、選択の検証、Canonical生成。
- `crates/mikomai-adapters/src/router_state.rs`: 最新観測の再利用、raw保存、Canonical化、Graph取り込み。
- `PortableGraph::latest_router_observation`: 装置/機能の最新観測を取得。元のsource_idと収集時刻を保持する。
- FFIの `get_state`: 上記31リソースを登録機器のcallback取得とCanonical化へ接続。Agentには検証済みCanonical JSONかエラーを返す。
- Swiftの `get_state`: 31リソースの読み取りコマンドの初期案を追加。Cisco系の一般的なshowコマンドを基準にしており、全ベンダー・機種に対応したコマンドとは扱わない。
- planner: 新規31リソースを `parameters.resource` の選択肢へ追加。
- 診断ログ: `router_state_request`, `router_canonicalization_request/response`, `router_graph_state`。ARPの診断ログと区別する。

NDPはlocalhostの既存処理を維持し、登録済みリモート機器は新しい処理へ接続する。未登録ターゲットやlocalhostとリモートの指定混同は拒否する。

## 処理と契約

1. 最新観測が未来時刻でなく20分以内なら再利用する。`refresh:true` で再取得できる。
2. Canonicalのversion、装置名、resource、収集時刻、フィールドを再検証する。有効なら取得・推論を省略する。
3. rawだけの新鮮な観測は再取得せずCanonical化する。失敗した取得や空出力も最新のraw-only観測として保存し、古いCanonicalを復活させない。再取得する場合は `refresh:true` を使う。
4. 原文のトークン、ラベル後の文字列、整数/実数、明示されたboolean、IP/MACの正規形、JSONの構造化値を候補にする。各候補に1始まりの原文行範囲を保持する。
5. LLMは `complete`, `empty_line`, `entries` を返す。各entryは原文の `start_line/end_line` と、全フィールドの候補インデックス（0始まり）またはnullを指定する。GBNFが出力構造・フィールド名・型に合う候補インデックスを制約する。
6. core相当の検証で候補範囲、原文範囲との対応、型、必須キー、値域、列挙値、未知フィールド、複合キー重複を確認する。検証エラーは最大4回の選択生成で修正を求め、成功しなければCanonicalを保存しない。
7. Canonical、normalized、候補・選択・原文行のevidenceをGraphに保存する。同じsource_id/収集時刻の観測を更新し、Canonical化で鮮度を延長しない。Graphの既存経路でネイティブノードと装置エッジを作る。

VRF、OSPF process/version、neighbor_id、SNMP/system名、portなど、原文にない必須識別子やデフォルト値は作らない。任意フィールドは未知なら省略する。型と出典の検証だけではLLMの意味解釈・全行網羅を完全には証明できないため、実ベンダーのfixturesを追加して精度を確認する。

空配列は、原文に `no entries`, `0 entries`, `no neighbors`, `0 neighbors`, `no records`, `0 records` の明示行があり、LLMが当該resourceの完全な空テーブルと判断した場合だけ認める。明示的なネイティブJSONの空配列も受け付ける。取得エラー、空文字、不明出力を空テーブルとして返さない。代表的なCLIエラーやページ送りマーカーも拒否する。

最大32KiB/512行/4096候補。超過時は切り捨てず失敗する。推論側のコンテキスト制限も切り捨てずエラーとなる。

## ネイティブJSONと返却例

すでにネイティブ表現のJSON（例: `{"dns_server":[{"address":"192.0.2.53"}]}`）なら、既存スキーマを検証し、LLMを使わず収集metadataを付ける。Canonical形式の入力にはmetadataの一致も必要。

```json
{
  "version": "router-constrained-index-v1",
  "metadata": {
    "source_device": "gw",
    "resource": "dns_server",
    "os_type": "cisco",
    "collected_at": "2026-10-06T00:00:00+00:00"
  },
  "dns_server": [{"address": "192.0.2.53"}]
}
```

`get_state` に `{"device":"gw","resource":"dns_server"}` を渡す。Graphへのnormalizedは従来どおり機能名の配列であり、metadataラッパーをネイティブテーブルに保存しない。

## 現段階の限界と精度向上箇所

- 生CLIの列位置に依存しない候補選択が初期実装。ベンダーごとの実出力と取得コマンドは未校正。必須キーがshow結果に含まれない場合は、必要な追加観測や明示的な収集contextを後続作業で定義する。
- `object` / `array` は原文中のJSON構造から選択する。生CLIの複数行からACL actions、QoS queues、NAT translationsなどのネスト構造を新規に組み立てる処理は未実装。任意の構造化フィールドは省略可能だが、原文の全情報が抽出できたことを意味しない。
- 秒/centiseconds/microseconds等の解釈ヒントは定義したが、単位変換・hex SPI変換は未実装。元の値が必要な単位/形式で表されていなければ選択しない。参照YANGではtelemetry sample_intervalはmilliseconds、heartbeat_intervalはseconds。
- GBNFを実行するGGUFを使用する。AFM用のtyped structured generation契約はこの新規処理には未追加のため、AFM選択時は明示的に失敗する。
- キャッシュは装置/機能単位で、取得コマンドは全体取得の初期案。機能内の追加scopeは未定義。
- nodesは既存Graphのupsertであり、完全インベントリ置換や消えたノードの削除は行わない。失敗した最新観測から `get_state` が古いCanonicalを返すことはないが、既存factsの削除は行わない。
- 実機SSH/serialとGUI表示は今回未検証。

実fixtureを増やす際は、各テーブルのraw/期待normalized/拒否条件を保存し、候補抽出と `resource_hint`、コマンド選択を調整する。ネスト構造の組み立てには、JSON候補以外でも葉の値を原文候補に結びつける契約を追加する。

## 検証

受け入れ条件: 全31テーブルで全フィールド型の再構成が可能、不正候補/型/必須キー/重複/根拠範囲を拒否、raw再試行とキャッシュが動作、取得時刻を保持、最新失敗から古い成功を復活させない、FFI callbackを経由してCanonical/factsを保存する。

```bash
cargo test -p mikomai-adapters router_ --lib
cargo test -p mikomai-adapters portable_device --lib
cargo test -p mikomai-core planner --lib
cargo test -p mikomai-ffi executor_ --lib
MIKOMAI_ROUTER_TEST_MODEL=/path/to/model.gguf cargo test -p mikomai-adapters real_llm_router_selection_grammar --lib -- --ignored --nocapture
swiftc -frontend -parse mikomai-desktop-mac/Sources/MikomaiDesktopMac/DesktopModel+Tools.swift
python3 scripts/generate-router-schema.py --check
```

結果と必須CLI JSONL確認は `doc/router-canonicalization-verification.md` に記録する。
