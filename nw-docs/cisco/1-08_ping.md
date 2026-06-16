---
brand: Cisco
os_version: IOS-XE
category: network
command_type: ping
target_model: Catalyst 9300
---

[Context: {brand} {target_model}, {category} {command_type} command reference]

## Command: `ping`

### Description

このコマンドを入力すると、ICMPエコー要求を送信して宛先との疎通確認ができます。
このコマンドはCiscoでのみ使えます。

### Usage

```text
#ping
```

### Output

```text
Type escape sequence to abort.
Sending 5, 100-byte ICMP Echos to 192.168.1.1, timeout is 2 seconds:
!!!!!
Success rate is 100 percent (5/5), round-trip min/avg/max = 1/2/4 ms
```
