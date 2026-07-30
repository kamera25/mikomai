---
brand: furukawa_fitelnet
os_version: FX-Sereis
category: configuration
command_type: hostname
target_model: F220 FX201 FX310
---

[Context: {brand} {target_model}, {category} {command_type} command reference]

## 概要
Fitelnetでの ホスト名の設定方法。

## 必須パラメータ
- hostname: ホスト名 (例: Hoge_router)

## 設定コマンドテンプレート
```config
hostname {{hostname}}
```