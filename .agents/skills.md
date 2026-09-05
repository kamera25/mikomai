# Mikomai 開発・検証スキルガイド (skills.md)

本ドキュメントは、**Antigravity** および **Codex** 等の AI コーディングアシスタントが本プロジェクトで作業する際に参照する共通のスキル・検証手順ガイドです。

---

## 必須検証ルール: 作業完了時の mikomai-cli chat 実行

コード修正、新機能実装、リファクタリング、設定変更などのタスクを実施した際は、**作業の最後に必ず `mikomai-cli` の `chat` コマンドを実行して、バックエンドパイプラインが正常に稼働することをエンドツーエンドで検証してください。**

Mikomai はデスクトップアプリ（Tauri / React）ですが、`mikomai-cli` は GUI と同一のコアランタイム（LLM、SurrealDB RAG、MCP ツール実行エンジン、AgentLoop 等）を共有しているため、CLI 経由で迅速かつ確実に機能検証を行えます。

---

## 検証コマンドと手順

### 1. 検証用 chat コマンドの実行

プロジェクトルートにて以下のコマンドを実行します。

```bash
# 基本実行（デフォルト: デバッグ出力なし、回答テキストのみ）
npm run cli -- chat "F220のVLAN設定方法を教えて"

# デバッグ・内部ログ（AgentLoop / MCP / LLMログ）を表示する場合
npm run cli -- chat "F220のVLAN設定方法を教えて" --debug
```

または JSON 形式で成否や応答データを確認する場合:

```bash
npm run cli -- --json chat "F220のVLAN設定方法を教えて"
```

### 2. 変更内容に応じた検証クエリの使い分け

変更したモジュールや機能に応じて、適切なプロンプトを選択して実行します。

| 対象コンポーネント | 検証プロンプト例 | 確認ポイント |
| :--- | :--- | :--- |
| **RAG / ナレッジ検索 / ドキュメント** | `npm run cli -- chat "F220のVLAN設定方法を教えて"`<br>`npm run cli -- chat "YAMAHA RTX1210 で LAN1 に 192.168.100.1/24 を設定するコマンドを教えて"` | SurrealDB からのドキュメント検索・引用、および LLM による回答生成が正しく動作するか |
| **機器接続 / MCP ツール / AgentLoop** | `npm run cli -- chat "NakaokuGW のインターフェース状態を確認して問題がないか要約して"` | `AgentLoop` による MCP ツールのディスパッチ、機器観測（Read-Only）、診断サマリーの出力が完了するか |
| **CLI / 出力フォーマット** | `npm run cli -- --json chat "FITELnet F220 の VLAN 設定手順の要約"` | JSON 出力（`{"ok": true, "data": ...}`）が正常にパース可能か |

---

## 成否の判定基準

1. **正常終了の確認**:
   - 期待される最終回答テキスト（または `--debug` 付与時に `[AgentLoop] Step X: Goal reached / Completed:` 等の内部ログ）が出力されていること。
   - 回答内容が質問（プロンプト）に対して妥当かつ完結していること。

2. **終了コードの確認**:
   - 正常に処理が完了した場合、コマンドは終了コード 0 で正常終了します。
   - 以前存在したプロセス終了処理時（Metal デバイス解放時）の `GGML_ASSERT` による終了コード 134 (abort) は修正済みであり、現在は正常終了（コード 0）することが期待されます。

---

## 関連ドキュメント
- CLI 全般マニュアル: `doc/mikomai-cli.md`
- Chat コマンド仕様: `doc/mikomai-cli-chat.md`
- エージェント設計: `doc/agent-architecture.md`
