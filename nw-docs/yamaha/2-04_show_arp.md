---
brand: Yamaha
os_version: Rev.15
category: arp
command_type: show
target_model: RTX1210 RTX1300
---

[Context: {brand} {target_model}, {category} {command_type} command reference]

## Command: `show arp`

### Description

このコマンドを入力すると、ARPテーブルを表示できます。
このコマンドはYamahaでのみ使えます。

### Usage

```text
#show arp
```

### Output

```text
Interface  IP Address      MAC Address        TTL(sec)
LAN1       192.168.100.2   00:11:22:33:44:55       900
```
