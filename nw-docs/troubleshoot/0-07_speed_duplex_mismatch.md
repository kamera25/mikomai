---
brand: 共通 全て
---
[Context: {brand} troubleshoot]

## Problem: `Speed/Duplexの不一致（Auto-Negotiation失敗）によるパフォーマンス低下`

### Solve
 1. スイッチのインターフェースステータスを確認し、エラーカウンタ（FCSエラーやコリジョンなど）が増加していないか確認する[cite: 1]。
 2. 対向機器との間で、Speed（速度）とDuplex（全二重/半二重）の設定が合致しているか確認する。
 3. 両端をAuto-Negotiationに設定するか、両端とも同じ固定値に設定する。

### Explain
 片方が固定設定（例: 1000/Full）で、もう片方がAutoの場合、Auto側はHalf-Duplexとしてリンクアップしてしまうことがあり、激しいパケットロスや通信速度の著しい低下を引き起こします。
