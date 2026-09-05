---
name: cli-verification
description: >-
  Use this skill to verify changes by running the mikomai-cli chat command at the end of development or bug-fixing tasks.
---

# mikomai-cli Chat Verification

作業完了時には、バックエンドパイプライン（LLM、SurrealDB RAG、MCPツール、AgentLoop）の動作をエンドツーエンドで確認するため、`mikomai-cli` の `chat` コマンドを実行して検証します。

詳細な手順とガイドラインは [.agents/skills.md](../../skills.md) を参照してください。

## 実行コマンド

```bash
# 基本実行（デフォルト: デバッグ出力なし、回答テキストのみ出力）
npm run cli -- chat "F220のVLAN設定方法を教えて"

# デバッグログを表示する場合
npm run cli -- chat "F220のVLAN設定方法を教えて" --debug

# エージェント応答結果を JSON のみで表示する場合
npm run cli -- chat "F220のVLAN設定方法を教えて" -j
```

## 確認ポイント
- 出力に適切な回答テキスト（または `-j` / `--json` 指定時はパース可能な JSON）が含まれていることを確認する（`--debug` 指定時は内部ログも確認可能）。

