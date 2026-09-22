---
name: cli-verification
description: >-
  Verify Mikomai development and bug fixes with mandatory mikomai-cli chat execution,
  change-specific acceptance criteria, and checks for behavior outside CLI coverage.
---

# mikomai-cli Chat Verification

コード修正・追加・バグ修正・設定変更の完了前に、必ず `mikomai-cli chat` を実行します。CLI の正常終了と変更した機能の正しさを別々に判定してください。

## 手順

1. 差分と実行経路を確認し、変更箇所を通る入力、期待結果、出てはいけない結果を実行前に決める。
2. CLI で確認できる範囲を特定し、範囲外の UI・LLM・MCP などには変更に応じた検証を選ぶ。
3. 最終変更後に基本の `chat` と変更に対応した検証を実行する。基本質問で変更箇所も確認できる場合は兼用する。
4. 終了コード・出力構造と、回答内容・実行された処理の証拠を分けて確認する。
5. 合格・不合格・未検証を検証項目ごとに報告し、未検証の機能まで成功と扱わない。

詳細な判定基準、実装上の制約、記録方法は [共通検証ガイド](../../skills.md) を参照してください。

## 基本コマンド

プロジェクトルートで実行します。

```bash
npm run cli -- chat "F220のVLAN設定方法を教えて"

# JSON 検証時は npm のバナーを抑制し、stdout と stderr を分けて取得する
npm run --silent cli -- chat "F220のVLAN設定方法を教えて" --json
```

現在の `npm run cli` は独立 CLI です。その成功だけでは GUI 側の LLM・SurrealDB・実機 MCP・AgentLoop の動作を証明できません。`ok: true` と回答の存在だけで検証を完了しないでください。
