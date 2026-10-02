# AI Agent Instructions for Mikomai (Antigravity & Codex)

本プロジェクト（Mikomai）におけるコーディングアシスタント（Antigravity、Codex 等）向けのガイドラインです。

## 作業完了時の必須検証ステップ
コードの変更、追加、バグ修正等の作業を行った際、および新機能・応答確認時は、タスク完了前に必ず `mikomai-cli` の `chat --debug-jsonl` コマンドを実行して動作検証を行ってください。通常テキスト出力や `--json` だけで代用してはいけません。

詳細な手順やプロンプト例は [.agents/skills.md](.agents/skills.md) に記載されています。

### 基本コマンド
```bash
npm run --silent cli -- chat "F220のVLAN設定方法を教えて" --debug-jsonl
```
stdout の全行をJSONとして解析し、内部処理と最後の `core_response` の `payload.status == 0`、期待通りの `payload.text` を確認してください。詳細なstderrログが必要な場合は `--debug` または `-d` を追加できます。

## 履歴・リスト選択行のクリック領域
履歴やリストの選択行は、文字やアイコンだけでなく行の全幅と上下の余白を含む領域をクリック可能にしてください。Button の label 内で全幅 frame、padding、`.contentShape(Rectangle())` を指定し、行高は最低 36pt を確保します。同種の選択行では既存の `HistorySelectionRow` を再利用してください。
