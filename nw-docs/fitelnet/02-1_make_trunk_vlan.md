---
brand: furukawa_fitelnet
os_version: FX-Sereis
category: configuration
command_type: trunk_vlan
target_model: F220 FX201 FX310
---

[Context: {brand} {target_model}, {category} {command_type} command reference]

## 概要
Fitelnetでの Trunk VLAN の追加方法。

## 必須パラメータとフォーマット
- interface_num: 物理インターフェース番号 (必ずスラッシュを含む形式。例: 1/1, 1/4)
- subinterface_num: サブインターフェース番号 (ドットの後に続く論理番号。通常は vlan_id と同値。例: 10, 20)
- vlan_id: VLAN ID (1-4094の数値)
- channel_group_num: チャネルグループ番号 (例: 10)

## LLM向けの推論・抽出ルール (重要)
1. 【文字列の除外と数値の抽出】ユーザー入力が「gigabitethernet1/1」や「ge1/1」などの場合、「gigabitethernet」などのアルファベット部分は除外すること。テンプレート側でインターフェース名（GigaEthernet）を補完するため、**数値とスラッシュの部分（例: 1/1）のみ**を厳密に抽出して `interface_num` とすること。
2. 【分割の禁止】抽出した数値部分（例: 1/1）は、絶対にスラッシュで分割せず、そのまま1つの値として `interface_num` に代入すること。
3. 【サブインターフェースの推論】ユーザー入力でサブインターフェース番号が明示されていない場合、設定対象の `vlan_id` の数値をそのまま `subinterface_num` に代入すること。

## 設定コマンドテンプレート
```config
interface GigaEthernet {{interface_num}}.{{subinterface_num}}
 vlan-id {{vlan_id}}
 channel-group {{channel_group_num}}
 exit
```