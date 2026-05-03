---
brand: Yamaha
os_version: Rev.15
category: routing
command_type: show
target_model: RTX1210 RTX1300
---
[Context: {brand} {target_model}, {category} {command_type} command reference]

## Command: `show ip route`

### Description
 このコマンドを入力すると、ルーティングテーブルを表示できます。
 このコマンドはYamahaでしか使えません。

### Usage

```text
#show ip route
```

### Output

```text
Destination         Gateway          Interface       Kind  Additional Info
default             -                PP[01]        static
192.168.100.0/24    192.168.100.1    LAN1        implicit
```
