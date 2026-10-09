# mikomai-core

Mikomai のアーキテクチャの中心に位置する、OS・UI・インフラに依存しない純粋なドメイン層クレートです。クリーンアーキテクチャの原則に基づき、DB、ファイル I/O、ネットワーク通信、UI、具象 LLM バックエンドへの依存を持たず、ビジネスロジックと外部契約（Port）のみを定義します。

## 主な役割と設計原則

- **I/O 非依存**: 外部リソース（ファイル、ソケット、SurrealDB、llama.cpp、OS API など）を直接扱わず、すべて `port` を通じて抽象化します。
- **共有ドメインモデル**: macOS アプリ（SwiftUI）、Windows アプリ（WinUI 3）、CLI のすべてで同一の業務ロジック・推論ロジック・検証スキーマを共有します。
- **安全な変更ゲート**: 機器設定変更やファイル転送などの重要操作は、すべてハッシュ固定された `OperationPlan` と `OperationGate` による二重承認・検証ゲートを通過させます。

---

## 主要モジュール構成

| モジュール | 役割と責務 |
| --- | --- |
| [`domain`](file:///Users/kamera25/mikomai/mikomai-core/src/domain.rs) | タスク（`Task` / `TaskSnapshot`）、変更計画（`OperationPlan`）、承認ゲート（`OperationGate`）、望ましい状態パッチ（`DesiredStatePatch`）、エビデンス（`Evidence`）などのドメインエンティティ。 |
| [`port`](file:///Users/kamera25/mikomai/mikomai-core/src/port.rs) | 外部インフラと接続するための抽象インターフェース群（`InferencePort`、`SearchPort`、`ToolExecutorPort`、`ReporterPort`、`TaskRepository` 等）。 |
| [`application`](file:///Users/kamera25/mikomai/mikomai-core/src/application.rs) | ユースケースサービス層（`ChatService`、`TaskManager`、`DiagnoseService`、`ChangeService`）。 |
| [`agent`](file:///Users/kamera25/mikomai/mikomai-core/src/agent.rs) | 自律調査エージェントのループ処理、ツール呼び出し判断、マルチステップ観測・思考・行動の制御。 |
| [`dispatch`](file:///Users/kamera25/mikomai/mikomai-core/src/dispatch.rs) | ユーザーの問い合わせ意図を判定し、ドキュメント解説応答（Worker）か実機調査（Agent）かを振り分けるディスパッチロジック。 |
| [`schema`](file:///Users/kamera25/mikomai/mikomai-core/src/schema) | ARP テーブル、ルーティングテーブル、インターフェース状態、パケット情報の正規化共通スキーマとバリデーション。 |
| [`nwdiag`](file:///Users/kamera25/mikomai/mikomai-core/src/nwdiag.rs) / [`plotter`](file:///Users/kamera25/mikomai/mikomai-core/src/plotter.rs) | ネットワーク構成図（nwdiag DSL）の構文検証、生成、AST 操作。 |
| [`redaction`](file:///Users/kamera25/mikomai/mikomai-core/src/redaction.rs) | パスワード、コミュニティ名、秘密鍵などの機密情報を LLM プロンプトやログから自動除去するマスキング処理。 |
| [`vision`](file:///Users/kamera25/mikomai/mikomai-core/src/vision.rs) | 添付画像（トポロジ図や画面キャプチャ）の安全制限検証（解像度・ファイルサイズ）とマルチモーダル解析契約。 |
| [`audit`](file:///Users/kamera25/mikomai/mikomai-core/src/audit.rs) | 不正改ざんを防止する監査ログレコード生成とハッシュ検証。 |

---

## 代表的な Port（インターフェース）

外部アダプター（`crates/mikomai-adapters` 等）が実装する主要なインターフェースです：

```rust
// LLM 推論インターフェース
pub trait InferencePort: Send + Sync {
    fn complete<'a>(&'a self, prompt: &'a str) -> PortFuture<'a, Result<String, String>>;
    fn capabilities(&self) -> InferenceCapabilities;
    fn availability(&self) -> ModelAvailability;
}

// ナレッジベース（RAG）検索インターフェース
pub trait SearchPort: Send + Sync {
    fn search<'a>(&'a self, query: &'a str, limit: usize) -> PortFuture<'a, Result<Vec<SearchHit>, String>>;
}

// ツール実行インターフェース
pub trait ToolExecutorPort: Send + Sync {
    fn execute<'a>(&'a self, tool_id: &'a str, args: &'a serde_json::Value) -> PortFuture<'a, Result<ToolResult, String>>;
}
```

---

## テスト実行

外部 I/O を持たないため、高速かつ決定論的に単体テストを実行できます。

```bash
cargo test -p mikomai-core
```
