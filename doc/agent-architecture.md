# エージェント指向バックエンド再設計

## 結論

ネットワーク操作の安全性と説明可能性を維持するため、バックエンドは
「LLM が提案する」「決定論的な中核が許可・実行・記録する」という構成にする。
LLM はネットワーク状態を直接更新せず、ツール実行や設定変更の権限も持たない。

## 現在の境界

`harness` は以下の責務に分ける。

```text
Swift UI / FFI
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

単純な `traceroute HOST`、`ping HOST`（count/size/df指定可）、`tnc HOST -port PORT` は、Coreの `dispatch::fast_route` がコマンド全体への一致で評価します。一致した入力を高confidence（1.0）の `fast_router` とし、FFIから既存の検証済みツールcallbackを直接呼びます。添付がない場合、成功時は資料検索・Agentタスク作成・Planner推論を行わず、既存の秘匿情報除去後の実行結果をそのまま通知します。ツール失敗時だけ失敗内容と実行引数をevidenceに保持してAgentへ引き継ぎ、ショートカットの自動重複実行を防ぎます。機器からの実行指定、追加分析・複数操作を含む依頼、添付ありの入力はAgentで処理します。

Standalone CLIの通常chatはWorker経路を使用するため、このFastRouter経路の検証にはFFI callbackのテストが必要です。

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
| `Reporter` | UI への進捗・最終結果を通知 | Swift FFI reporter |

`AgentLoop` はこれらのポートを受け取り、`plan -> authorize -> execute -> record` の
順序だけを管理する。各ポートはフェイク実装に置換できるため、LLM・Tauri・実機なしで
シナリオテストを実行できる。

## Core・推論adapter・FFIの責務

- `mikomai-core::agent::AgentPlanner` がショートカット、対象選択、Plannerプロンプト、Decision検証、完了・質問・承認要求の判断を管理します。
- `mikomai-core::response` が通常回答のプロンプトと完了事実からの報告生成を管理します。報告生成が失敗しても機器操作を再実行せず、完了事実を返します。
- Coreの `InferencePort` / `StreamingInferencePort` / `VisionPort` / `OperationProposalPort` は外部実装への契約です。`ReporterPort` は生成担当ではなく、進捗と結果の通知担当です。
- `mikomai-adapters::local_llama` がGGUFモデルの寿命、サンプリング、キャンセル、llama.cpp MTMDによる画像エンコード・推論を所有します。Coreはllama.cppやモデルの実体に依存しません。
- FFIの推論・Planner入口はC引数の変換、ポートの組立て、Core呼出、Swift callback変換を担当します。LLMの判断・回答生成プロンプトはFFIに置きません。既存の機器ツール・監査・保存先の入口処理は引き続きFFIにあります。

### Vision

Swiftの画像添付は `__MIKOMAI_ATTACHMENTS_V1__` を先頭につけたJSONで、UTF-8の `text` と、`name` / `mimeType` / `base64` を持つ `images` を渡します。従来のテキスト添付ABIも利用できます。PNG/JPEGのみ、1ファイル8 MiB・合計16 MiB・4画像・各画像16,777,216画素を上限とします。

Coreは画像の形式・サイズを検査し、`VisionPort`へ実際の画像bytesを渡します。解析結果は「添付画像からの推定・非信頼資料」として回答とPlannerへ渡し、実機から得た確認済み状態とは区別します。不正画像、未設定mmproj、非対応モデル、画像解析失敗はエラーにし、画像を見たような代替文章は生成しません。画像とテキストがコンテキスト長を超える場合は画像を捨てず、縮小等を求めるエラーを返します。

設定の `visionEnabled` / `mmprojPath` は `mikomai_configure_vision` からadapterへ渡します。現在のローカルadapterはGemma 4のチャット形式に対応し、画像対応GGUFと、そのモデルに対応したmmprojの両方が必要です。解析済み添付資料は推論由来のevidenceとしてtask snapshotに保持し、ユーザー選択からの再開時にも引き継ぎます。`MIKOMAI_N_GPU_LAYERS=0` の場合はGPU deviceも明示的に除外してCPUを使用します。

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
4. ~~フェイクポートを用いた「調査成功」「ポリシー拒否」「承認待ち」「ツール失敗」のシナリオテストを追加する。~~ 共通ロジックはRust coreのテストで検証する。

この設計はTauriランタイムを前提にせず、複数エージェントや長時間タスクへ拡張できる。


### Planner の部分グラフ取得

Planner は `OBSERVE` で次のツールを実行できます。

```json
{
  "action_type": "OBSERVE",
  "objective": "R1とR2周辺の関係を調べる",
  "tool": "get_subgraph",
  "target": null,
  "parameters": {
    "roots": ["R1", "R2"],
    "depth": 2,
    "relations": ["interface", "bgp", "vrf", "route"]
  }
}
```

`roots` は保存済み機器名（1〜32件）、`depth` は0〜8、`relations` は上記の種類から1つ以上指定します。選択した関係だけを双方向に幅優先探索し、起点から指定ホップ数以内の `nodes` と探索した `edges`、未登録の `missing_roots` を返します。深さ0では起点ノードのみを返し、重複する起点・ノード・辺はまとめます。ノードの `node_id` が辺の `from` / `to` に対応します。

既存の `has_interface`、`device_has_route` はそれぞれ `interface`、`route` として扱います。BGP・VRFは保存済みの `bgp` / `device_has_bgp`、`vrf` / `device_has_vrf` の辺を対象にします。BGP・VRFの新しい収集・正規化処理はこのツールには含みません。保存されていない関係は返らないため、空の結果はネットワーク上の関係が存在しない証拠にはなりません。

実機への問い合わせや自動更新は行いません。保存済みレコードの観測時刻と出典を保持するので、鮮度が必要な場合は `get_state` 等で別途収集してください。1000ノードまたは2000辺を超える探索はエラーとして返します。


## TCPポートチェック

`NakaokuGW の22/tcpが空いているかチェック`、`192.0.2.1のTCPポート443を確認して`、`tnc 192.0.2.1 -port 22` のような単独の明示要求はFastRouterで実行する。登録機器名は登録IPに解決し、このコンピュータから対象へのTCP接続を試す。CLIでもIP/DNSへの単独確認を実行できる（CLIにはデスクトップの登録機器一覧は渡らない）。

複数ポート、複数対象、Pingとの併用、原因調査はAgentが順に読み取りツールを選択する。`self_network_test_connection`（別名 `self_network_test_net_connection`）の引数は `host` と整数の `port`（1〜65535）、任意の `protocol: tcp`。UDPは未対応でTCP結果による代用を禁止する。TCP接続成功は到達可能性の観測であり、接続失敗だけで閉鎖やフィルタの原因を断定しない。
