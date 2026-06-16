---
brand: Cisco
os_version: IOS-XE
category: switching
command_type: show
target_model: Catalyst 9300
---

[Context: {brand} {target_model}, {category} {command_type} command reference]

## Command: `show mac address-table`

### Description

このコマンドを入力すると、スイッチが学習したMACアドレステーブルを表示できます。
このコマンドはCiscoでのみ使えます。

### Usage

```text
#show mac address-table
```

### Output

```text
          Mac Address Table
-------------------------------------------

Vlan    Mac Address       Type        Ports
----    -----------       --------    -----
   1    0000.1111.2222    DYNAMIC     GigabitEthernet1/0/1
```
