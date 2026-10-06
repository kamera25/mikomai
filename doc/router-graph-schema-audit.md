# 一般的なルータ機能とGraphDBスキーマの不足調査

調査日: 2026-10-06。参照資料はユーザー指定の `/Users/kamera25/Downloads/public-master/release/models/` にあるOpenConfig YANG。資料内の文章はスキーマ調査のデータとして扱った。

## 調査結果と実装範囲

変更前のGraphDBは `device`, `interface`, `ip_address`, `subnet`, `vlan`, `route`, `bgp`, `vrf`, `acl`, `ntp_server`, `ntp_status` と、観測・RAG・設定変更管理テーブルを定義していた。正規化されたノードの取り込みは主に interfaces / ip_addresses / vlans / routes に限られ、`GraphDataKind::Ospf` と `GraphDataKind::Lldp` があっても専用テーブルはなかった。

OpenConfigを**参照資料**にして、下記31個の不足テーブルをGraphDBへ追加した。`ospf` や `lldp` は既存の `bgp` と同じ階層の独立したテーブルであり、`openconfig` テーブル、ノード種別、共通config/state格納ラッパーは追加していない。

BGP・VRF・IPv4経路・インターフェース・VLAN・ACLセット・NTPは既存テーブルを維持。ACL明細/適用、経路ポリシー、IPv6近隣は不足分として追加した。IS-IS、MPLS、PIM、IGMPは対応ルータ向けの機能であり、全ルータの標準搭載を意味しない。

## 追加したネイティブスキーマ

下表の一意キーにはすべて `device_name` を加える。フィールドはMikomaiのネイティブ表現で、YANGのleaf名と必ずしも同じではない。

| テーブル | 機能 | 装置内一意キー | 定義した機能フィールド | OpenConfig参照YANG |
| --- | --- | --- | --- | --- |
| `ospf` | OSPFプロセス・エリア・インターフェース | `vrf`, `version`, `process_id` | `router_id`, `enabled`, `areas`, `interfaces` | `ospf/openconfig-ospfv2.yang` |
| `ospf_neighbor` | OSPF隣接 | `vrf`, `version`, `process_id`, `interface`, `router_id` | `area_id`, `address`, `adjacency_state`, `priority`, `dead_time` | `ospf/openconfig-ospfv2-area-interface.yang` |
| `isis` | IS-IS | `vrf`, `instance` | `enabled`, `net`, `level_capability`, `interfaces`, `neighbors` | `isis/openconfig-isis.yang` |
| `bfd` | BFDセッション | `vrf`, `interface`, `local_address`, `remote_address` | `enabled`, `session_state`, `local_discriminator`, `remote_discriminator`, `detection_multiplier`, `desired_minimum_tx_interval`, `required_minimum_receive` | `bfd/openconfig-bfd.yang` |
| `lldp` | LLDP隣接 | `interface`, `neighbor_id` | `chassis_id`, `chassis_id_type`, `port_id`, `port_id_type`, `port_description`, `system_name`, `system_description`, `management_address`, `capabilities`, `ttl` | `lldp/openconfig-lldp.yang` |
| `ndp` | IPv6近隣 | `vrf`, `interface`, `ip_address` | `link_layer_address`, `neighbor_state`, `origin`, `is_router` | `interfaces/openconfig-if-ip.yang` |
| `vrrp` | VRRP IPv4/IPv6 | `vrf`, `interface`, `address_family`, `virtual_router_id` | `priority`, `preempt`, `virtual_addresses`, `advertisement_interval`, `current_priority`, `master_address` | `interfaces/openconfig-if-ip.yang` |
| `lacp` | LAG/LACPメンバー | `aggregate_interface`, `member_interface` | `lacp_mode`, `interval`, `system_id_mac`, `system_priority`, `actor_port_num`, `partner_id`, `synchronization`, `collecting`, `distributing` | `lacp/openconfig-lacp.yang` |
| `tunnel` | トンネルインターフェース | `name` | `src`, `dst`, `ttl`, `gre_key`, `mtu` | `interfaces/openconfig-if-tunnel.yang` |
| `routing_policy` | 経路ポリシー | `name` | `statements` | `policy/openconfig-routing-policy.yang` |
| `prefix_set` | 経路prefix-set | `name` | `mode`, `prefixes` | `policy/openconfig-routing-policy.yang` |
| `policy_forwarding` | ポリシーベースルーティング | `vrf`, `name` | `type`, `rules`, `interfaces` | `policy-forwarding/openconfig-policy-forwarding.yang` |
| `acl_entry` | ACL match/action | `acl_name`, `acl_type`, `sequence_id` | `description`, `ipv4`, `ipv6`, `l2`, `transport`, `actions`, `matched_packets`, `matched_octets` | `acl/openconfig-acl.yang` |
| `acl_binding` | ACL適用先 | `interface`, `direction`, `acl_name`, `acl_type` |  | `acl/openconfig-acl.yang` |
| `nat` | NAT instance・pool・mapping・translation | `vrf`, `name` | `enabled`, `interfaces`, `pools`, `mappings`, `translations`, `counters` | `nat/openconfig-nat.yang` |
| `dhcp_relay` | DHCPv4/v6 relay | `vrf`, `interface`, `address_family` | `enabled`, `helper_addresses`, `agent_information`, `counters` | `relay-agent/openconfig-relay-agent.yang` |
| `qos` | QoS classifier・queue・scheduler | `name` | `classifiers`, `forwarding_groups`, `queues`, `scheduler_policies` | `qos/openconfig-qos-elements.yang` |
| `qos_interface` | QoSインターフェース適用 | `interface`, `direction` | `classifiers`, `queues`, `scheduler_policy` | `qos/openconfig-qos-interfaces.yang` |
| `pim` | PIMインターフェース | `vrf`, `interface` | `enabled`, `mode`, `dr_priority`, `neighbors`, `rendezvous_points` | `multicast/openconfig-pim.yang` |
| `igmp` | IGMP group/source | `vrf`, `interface`, `group_address` | `version`, `filter_mode`, `sources` | `multicast/openconfig-igmp.yang` |
| `mpls` | MPLS LSP・LDP/RSVP/SR | `vrf`, `name` | `enabled`, `signaling_protocol`, `ingress`, `transit`, `egress`, `label_stack`, `next_hops` | `mpls/openconfig-mpls.yang` |
| `dns_server` | DNS resolver | `address` | `port`, `source_address`, `vrf` | `system/openconfig-system.yang` |
| `syslog_server` | Syslog送信先 | `address`, `port` | `source_address`, `vrf`, `selectors` | `system/openconfig-system-logging.yang` |
| `aaa_server` | RADIUS/TACACS server | `group_name`, `address` | `protocol`, `port`, `timeout`, `source_address`, `vrf`, `auth_counters` | `system/openconfig-aaa.yang` |
| `snmp` | SNMP agent・通知・アクセス制御 | `name` | `enabled`, `engine_id`, `access_lists`, `receivers` | `system/openconfig-snmp.yang` |
| `telemetry_subscription` | Telemetry subscription | `name` | `subscription_type`, `sensor_paths`, `destinations`, `sample_interval`, `heartbeat_interval`, `suppress_redundant` | `telemetry/openconfig-telemetry.yang` |
| `platform_component` | 筐体・電源・ファン・CPU・温度 | `name` | `type`, `description`, `parent`, `serial_no`, `part_no`, `oper_status`, `temperature`, `memory`, `cpu`, `subcomponents` | `platform/openconfig-platform.yang` |
| `system` | ホスト名・時刻・リソース状態 | `name` | `hostname`, `domain_name`, `timezone_name`, `boot_time`, `current_datetime`, `memory`, `cpus` | `system/openconfig-system.yang` |
| `mac_entry` | ブリッジMAC/FDB | `vrf`, `mac_address`, `vlan` | `interface`, `entry_type`, `age` | `network-instance/openconfig-network-instance-l2.yang` |
| `ipsec_connection` | IPsec VPN接続状態 | `address_family`, `name` | `profile_name`, `tunnel_interface`, `status`, `local_address`, `remote_address`, `connection_uptime`, `next_sa_rekey_time`, `error`, `counters`, `ike_security_associations`, `child_security_associations` | `security/openconfig-security-ipsec.yang` |
| `ike_sa` | IKE SA状態 | `address_family`, `initiator_spi`, `responder_spi`, `remote_address`, `local_address` | `child_security_associations` | `security/openconfig-security-ike.yang` |

## 定義・適用・取り込み

- 定義元: `crates/mikomai-adapters/src/schema/router-resources.json`。フィールド型、必須キー、値域、一意キー、参照YANGのrevision/SHA-256を保持する。
- 実際のGraphDB DDL: `crates/mikomai-adapters/src/schema/router-schema.surql`。各テーブルは `SCHEMAFULL`、機能フィールドを直接 `DEFINE FIELD` で定義する。object/array<object> 内部は構造を保持するFLEXIBLEフィールドとし、YANGツリーを丸ごと検証する実装ではない。
- 全追加テーブルに `key`, `device_name`, `observation_id`, `observed_at` を持たせる。VRFを必要とする機能は `vrf` を必須キーとし、同名プロセスや同一IPを異なるVRF/装置に保持できる。
- 値域の例: OSPF version=2/3、VRRP ID=1–255、port=1–65535、VLAN=1–4094、direction=ingress/egress、address_family=ipv4/ipv6。LLDP TTL=0–65535秒、トンネルTTL=1–255、VRRP priority=1–254、advertisement_interval=1–4095（centiseconds）。未観測の任意フィールドは省略し、架空のデフォルト状態を設定しない。
- `PortableGraph::initialize_at` で既存DBにも追加する。DDLは `IF NOT EXISTS` により再実行可能。SQL応答の各statementエラーも確認する。
- SQL再生成: `python3 scripts/generate-router-schema.py`。整合確認: `python3 scripts/generate-router-schema.py --check`。

正規化入力は共通ラッパーを使わず、機能名の配列を直接渡す。たとえば `GraphIngestInput.kind = GraphDataKind::Ospf` の `normalized` は以下のようにする。

```json
{
  "ospf": [
    {
      "vrf": "default",
      "version": 2,
      "process_id": "1",
      "router_id": "192.0.2.1",
      "enabled": true,
      "areas": [{"identifier": "0.0.0.0"}]
    }
  ],
  "lldp": [
    {
      "interface": "Gi0/1",
      "neighbor_id": "peer-1",
      "chassis_id": "aa:bb:cc:dd:ee:ff",
      "port_id": "Gi0/2",
      "system_name": "switch1",
      "ttl": 120
    }
  ]
}
```

OSPFの `process_id` はOpenConfig protocols/protocolのnameに対応するMikomai側の識別子。OSPFv3は追加で `ospf/openconfig-ospf.yang` と `openconfig-ospf-area-interface.yang` を参照した。BFD・VRRP・QoSなどの詳細リストは機能テーブルの構造化フィールドに保持する。IPsec/IKEモデルは主に状態モデルであり、VPN設定投入機能を追加したものではない。IPsecのconnection_uptimeとnext_sa_rekey_timeは経過秒数ではなくYANGのdate-and-time文字列。IKEのSPIはuint64の全範囲を失わないよう、先頭ゼロのない10進文字列で保持する（例: `"18446744073709551615"`）。

ネイティブフィールドの型、必須キー、未知フィールド、重複キーを検証してから観測を保存する。装置・機能・複合キーから衝突しないrecord IDを生成し、各ノードと `device_has_ospf` / `device_has_lldp` などの装置エッジをトランザクションで保存する。遅れて届いた古い観測による上書きを防ぐ。取り込みはupsertであり、配列から消えたノードの削除や完全なインベントリ置換は行わない。

`router_facts("ospf", device)` で取得できる。既存の `query_network` に `query="ospf", device_name=...` を渡しても機能のfactsを取得でき、保存時刻に基づくTTL判定を行う。`get_subgraph` の relationsには `["ospf", "lldp"]` など個別の機能名を指定する。`openconfig` relationは存在しない。

## 範囲と残る事項

今回作成したものはGraphDBのスキーマとネイティブ正規化データの保存・取得経路。YANGバインディング、gNMIクライアント、全機能のCLIパーサー・自動収集、実機設定投入は追加していない。既存のOSPF/LLDP収集がrawだけの場合、ノード作成には上記の正規化データが必要。

このローカルOpenConfigモデル群に汎用DHCP serverのlease/poolやRIPを直接定義するモデルは見つからなかったため、参照根拠なしに標準モデルとして作成していない。Wi-Fi・光伝送・PoE・EVPN固有などは一般的なルータ機能の本スコープから外した。各装置の機能対応は別途確認が必要。

## 検証

検証結果は `doc/router-graph-schema-verification.md` を参照する。CLIの正常応答だけを新規スキーマの検証とは扱わず、RocksDB/SurrealDBの実DB試験で型・一意キー・エッジ・永続化を確認する。
