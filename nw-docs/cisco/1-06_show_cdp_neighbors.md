---
brand: Cisco
os_version: IOS-XE
category: cdp
command_type: show
target_model: Catalyst 9300
---

[Context: {brand} {target_model}, {category} {command_type} command reference]

## Command: `show cdp neighbors`

### Description

このコマンドを入力すると、CDP（Cisco Discovery Protocol）で検出された隣接機器の情報を表示できます。
このコマンドはCiscoでのみ使えます。

### Usage

```text
#show cdp neighbors
```

### Output

```text
Device ID        Local Intrfce     Holdtme    Capability  Platform  Port ID
SwitchB          Gig 1/0/1         120        S I         C9300     Gig 1/0/2
```
