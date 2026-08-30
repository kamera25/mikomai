# エージェント指向バックエンド再設計

## 結論

ネットワーク操作の安全性と説明可能性を維持するため、バックエンドは
「LLM が提案する」「決定論的な中核が許可・実行・記録する」という構成にする。
LLM はネットワーク状態を直接更新せず、ツール実行や設定変更の権限も持たない。

## 現在の境界

`harness` は以下の責務に分ける。

```text
UI / Tauri command
        |
        v
Request dispatcher --- Worker (単発の説明・生成)
        |
        v
Agent orchestrator --- Planner port (LLM の Decision 提案)
        |                 |
        |                 v
        |             Policy + schema gate
        |                 |
        v                 v
Event-sourced NetworkState <- Tool executor port -> MCP / device
```

- `intent`: UI の振り分けと Agent 内の変更判定で同じ規則を使う。
- `execution`: `Decision` から実際のツール引数と `Observation` を作る純粋関数。
- `state_machine`: 不正なフェーズ遷移と上限超過を拒否する。
- `NetworkState`: 直接観測と Action の実行結果を区別してイベント化し、ログから再生できる。

今回のリファクタリングでは、`intent`、`execution`、状態遷移、ActionResult の因果記録に
加え、`PlannerPort`、`ToolExecutorPort`、`ReporterPort` と既存実装のアダプタを実装している。

## 目標アーキテクチャ

次の段階では `AgentLoop` をオーケストレーションだけに薄くし、外部依存をポート化する。

| ポート | 責務 | 実装例 |
| --- | --- | --- |
| `Planner` | NetworkState から Decision を提案 | LLM planner / ルールベース fallback |
| `ActionAuthorizer` | スキーマ、ポリシー、承認計画を検査 | SchemaValidator + PolicyValidator |
| `ToolExecutor` | 許可済み Action を実行 | MCP executor |
| `EventStore` | Goal/Decision/Action/Result を永続化・再生 | 現在の EventLog、後に DB |
| `Reporter` | UI への進捗・最終結果を通知 | Tauri Emitter |

`AgentLoop` はこれらのポートを受け取り、`plan -> authorize -> execute -> record` の
順序だけを管理する。各ポートはフェイク実装に置換できるため、LLM・Tauri・実機なしで
シナリオテストを実行できる。

## 安全上の不変条件

1. `CONFIGURE` と `ROLLBACK` は承認済みの Operation Plan なしに ToolExecutor へ渡さない。
2. 実行された引数、対象、出力、成否は同じ Action と関連付けて必ず記録する。
3. 再実行時は EventLog の replay だけで Observed State を復元できる。
4. Planner の不正 JSON、未知ツール、上限超過は外部操作を起こさず終了または人手確認にする。
5. Builder Co-Worker の結果は Observation として扱い、同じ変更操作を自動再開しない。

## 段階的な移行

1. ~~`AgentLoop` から `Planner` / `ToolExecutor` / `Reporter` trait を抽出し、既存実装を adapter にする。~~ 完了
2. ~~`EventLog` をタスク ID 単位で永続化し、開始・再開・監査表示を replay に統一する。~~ タスクごとの永続化と replay を実装済み。再開・監査 UI は次の UI 段階で接続する。
3. Action の idempotency key、タイムアウト、キャンセル、リトライ方針を ActionResult に追加する。
4. フェイクポートを用いた「調査成功」「ポリシー拒否」「承認待ち」「ツール失敗」のシナリオテストを追加する。

この順序なら、既存の Tauri/MCP 境界を壊さずに、複数エージェントや長時間タスクへ拡張できる。
