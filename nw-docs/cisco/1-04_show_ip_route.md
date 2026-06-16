---
brand: Cisco
os_version: IOS-XE
category: routing
command_type: show
target_model: Catalyst 9300
---

[Context: {brand} {target_model}, {category} {command_type} command reference]

## Command: `show ip route`

### Description

このコマンドを入力すると、ルーティングテーブルを表示できます。
このコマンドはCiscoでのみ使えます。

### Usage

```text
#show ip route
```

### Output

```text
Codes: L - local, C - connected, S - static, R - RIP, M - mobile, B - BGP
Gateway of last resort is not set

      192.168.1.0/24 is variably subnetted, 2 subnets, 2 masks
C        192.168.1.0/24 is directly connected, GigabitEthernet1/0/1
L        192.168.1.1/32 is directly connected, GigabitEthernet1/0/1
```
