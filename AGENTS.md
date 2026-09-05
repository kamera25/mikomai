# AI Agent Instructions for Mikomai (Antigravity & Codex)

本プロジェクト（Mikomai）におけるコーディングアシスタント（Antigravity、Codex 等）向けのガイドラインです。

## 作業完了時の必須検証ステップ
コードの変更、追加、バグ修正等の作業を行った際は、タスク完了前に必ず `mikomai-cli` の `chat` コマンドを実行して動作検証を行ってください。

詳細な手順やプロンプト例は [.agents/skills.md](.agents/skills.md) に記載されています。

### 基本コマンド
```bash
npm run cli -- chat "F220のVLAN設定方法を教えて"
```
期待通りの最終回答テキストが出力されることを確認してください（内部ループやデバッグログを確認する場合は `--debug` または `-d` を付与可能）。
