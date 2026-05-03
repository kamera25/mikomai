---
brand: Cisco
os_version: IOS-XE
category: interface
command_type: show
target_model: Catalyst 9300
---
[Context: {brand} {target_model}, {category} {command_type} command reference]

## Command: `show interface status`

### Description
 このコマンドを入力すると、インターフェースのステータスを一覧表示できます。
 このコマンドはCiscoでしか使えません。

### Usage

```text
#show interface status
```

### Output

```text
D
show interface status
               Connected to                Vlan      Duplex  Speed Type
GigabitEthernet1/0/1     connected        10      a-full a-1000 1000Base-SX
GigabitEthernet1/0/2     connected        20      a-full a-1000 1000Base-SX
GigabitEthernet1/0/3     notconnect                    auto    auto 1000Base-SX
GigabitEthernet1/0/4     notconnect                    auto    auto 1000Base-SX
```
