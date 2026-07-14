---
brand: furukawa_fitelnet
os_version: FX-Sereis
category: configuration
command_type: configuration
target_model: F220 FX201 FX310
---

[Context: {brand} {target_model}, {category} {command_type} command reference]

## 概要
FitelnetでのVLANの追加方法。

## 必須パラメータ
- interface_num: 物理インターフェース番号 (例: 1/4)
- subinterface_num: サブインターフェース番号 (例: 10)
- vlan_id: VLAN ID (1-4094)
- channel_group_num: チャネルグループ番号 (例: 10)

## 設定コマンドテンプレート
```fitelnet
interface GigaEthernet {{interface_num}}.{{subinterface_num}}
 vlan-id {{vlan_id}}
 channel-group {{channel_group_num}}
 exit
```