---
brand: Cisco
os_version: IOS-XE
category: network
command_type: traceroute
target_model: Catalyst 9300
---
[Context: {brand} {target_model}, {category} {command_type} command reference]

## Command: `traceroute`

### Description
 このコマンドを入力すると、宛先までの通信経路（経由するルータ）を確認できます。
 このコマンドはCiscoでしか使えません。

### Usage

```text
#traceroute
```

### Output

```text
Type escape sequence to abort.
Tracing the route to 192.168.1.1
VRF info: (vrf in name/id, vrf out name/id)
  1 192.168.0.254 1 msec 1 msec 1 msec
  2 192.168.1.1 2 msec 2 msec 2 msec
```
