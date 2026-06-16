---
brand: Yamaha
os_version: Rev.15
category: configuration
command_type: show
target_model: RTX1210 RTX1300
---

[Context: {brand} {target_model}, {category} {command_type} command reference]

## Command: `show config`

### Description

このコマンドを入力すると、現在稼働中の設定ファイルを表示できます。
このコマンドはYamahaでしか使えません。

### Usage

```text
#show config
```

### Output

```text
# RTX1210 Rev.14.01.40 (Thu Nov 11 11:11:11 2021)
# MAC Address : 00:a0:de:11:22:33, 00:a0:de:11:22:34, 00:a0:de:11:22:35
# Memory 256Mbytes, 2LAN, 1BRI
# main:  RTX1210 ver=c00 mac=00:a0:de:11:22:33 board=0012
#
ip lan1 address 192.168.100.1/24
```
