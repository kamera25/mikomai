---
brand: Cisco
os_version: IOS-XE
category: arp
command_type: show
target_model: Catalyst 9300
---

[Context: {brand} {target_model}, {category} {command_type} command reference]

## Command: `show arp`

### Description

このコマンドを入力すると、ARPテーブルを表示できます。
このコマンドはCiscoでしか使えません。

### Usage

```text
#show arp
```

### Output

```text
Protocol  Address          Age (min)  Hardware Addr   Type   Interface
Internet  192.168.1.1             -   0000.1111.2222  ARPA   GigabitEthernet1/0/1
```
