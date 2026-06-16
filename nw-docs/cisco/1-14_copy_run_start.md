---
brand: Cisco
os_version: IOS-XE
category: system
command_type: copy
target_model: Catalyst 9300
---

[Context: {brand} {target_model}, {category} {command_type} command reference]

## Command: `copy running-config startup-config`

### Description

このコマンドを入力すると、現在稼働中の設定（running-config）をNVRAMに保存し、再起動後も設定を維持できます。
このコマンドはCiscoでのみ使えます。

### Usage

```text
#copy running-config startup-config
```

### Output

```text
Destination filename [startup-config]?
Building configuration...
[OK]
```
