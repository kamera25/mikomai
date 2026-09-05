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
npm run cli -- chat "F220のVLAN設定方法を教えて"
```

## 確認ポイント
- 出力に `[AgentLoop] Step X: Goal reached / Completed:` および適切な回答テキストが含まれていることを確認する。
