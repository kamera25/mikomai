# Coordinator と Fast Agent の設計

## 目的

Worker の追加ごとに AgentLoop へ個別の分岐を増やさず、完了、追加入力、承認、引継ぎ、失敗を同じ規約で扱う。

Planner は「次に何をするか」と「処理が完了したか」を判断する。ユーザー向けの最終回答、承認待ち、Worker の引継ぎは Coordinator が交通整理する。これにより、Planner がツール実行や対話文の生成を兼務しない構成にする。

## 責務分離

```text
ユーザー入力
    |
    v
Fast Router（LLMを必要としない、エキスパートシステム的な単純な動作、ルールベースの入口振り分けを行う）
    |
    +----------------------------> 単発完了パス（Fast Agentを呼び出す）
    |
    v
Planner ------------------------> Decision
    |                                |
    |                                v
    |                         AgentLoop / Policy / MCP
    |                                |
    |                                v
    +<------------------------- WorkerOutcome
                                     |
                                     v
                               Coordinator
                 +-------------------+-------------------+
                 |                   |                   |
                 v                   v                   v
            Fast Agent         ユーザー入力待ち       承認待ち
                 |
                 v
          ユーザー向け最終回答
```

Fast Router は既存の決定論的な入口振り分けである。LLM に依存せず、Ping など既知の単発要求を単発完了パスへ渡す。複数ステップの判断、観測の統合、承認が必要な要求は Planner へ渡す。Coordinator や Fast Agent の代替ではなく、その前段に置く。

| コンポーネント | 責務 | 行わないこと |
| --- | --- | --- |
| Fast Router | Ping など既知の単発要求を決定的に振り分ける | LLM 推論、複数ステップの計画、承認判断 |
| Planner | 観測結果から次の `Decision`、または完了宣言を作る | 最終会話文の生成、実行権限の付与 |
| AgentLoop | `Decision` の検証、実行、観測の記録、状態遷移 | Worker ごとの対話ロジックを持つこと |
| Coordinator | Worker の共通結果を次の処理に決定的に振り分ける | LLM による判断、機器操作 |
| Fast Agent | 完了事実メモを利用者向けの簡潔な報告に整形する | ツール呼び出し、再試行、設定変更、実行フロー変更 |
| Worker | 限定された専門処理を実行し、共通結果を返す | 任意のネットワーク操作、直接の UI 制御 |

## 共通 Worker 契約

`src-tauri/src/harness/coordinator.rs` に次の戻り値を定義する。

| `WorkerOutcome` | 意味 | Coordinator の遷移 |
| --- | --- | --- |
| `Completed` | 処理が終わり、事実メモを返せる | `PresentWithFastAgent` |
| `AwaitingUserInput` | 必須の値または選択が不足 | `AskUser` |
| `AwaitingApproval` | 実行前に明示承認が必要 | `AwaitApproval` |
| `Handoff` | 次の専門 Worker に渡す | `DispatchWorker` |
| `Failed` | 再試行せず利用者へ失敗を伝える | `Fail` |

`Completed` の `completion_brief` は、処理の事実、根拠、制約だけを含める。利用者への挨拶、操作提案、ツール JSON、生パケット、秘密情報は含めない。

## 現在の実装

### Planner の完了宣言

Planner が `FINISH` を選ぶ場合、`final_answer` は最終回答ではなく完了事実メモとして扱う。AgentLoop はこれを `Coordinator::after_planner_terminal` へ渡す。

Coordinator が `PresentWithFastAgent` を返した場合、`ToolExecutorPort::present_completion` が Fast Agent を呼ぶ。Fast Agent は固定のシステム指示で、ツール呼び出し・再試行・設定変更をしない。Fast Agent が利用できない場合だけ、元の事実メモを返す。このフォールバックは既に完了した操作を再実行しない。

### Builder Co-Worker

Builder の結果は `builder_handoff_outcome` で `WorkerOutcome` に変換される。

- 不足項目を示す結果は `AwaitingUserInput` となり、ユーザー確認で停止する。
- 完了、キャンセル、投入失敗などの終端結果は `Completed` となり、Fast Agent が報告する。

Builder 完了後に AgentLoop がネットワーク操作、RAG、Builder を自動で再実行してはならない。

## Packet Safety Worker への適用

Packet Safety Worker は `src-tauri/src/harness/packet_safety.rs` に実装され、`network_packet_safety` MCPツールとして Agent と Fast Router から呼び出せる。LLM に送信パラメータや再試行を決定させず、許可された intent だけを受け付ける。

| 状況 | 戻り値 |
| --- | --- |
| 対象インターフェース、VLAN、DHCP 識別子が不足 | `AwaitingUserInput` |
| パケットの解析またはプレビューが完了 | `Completed` |
| 実送信に必要な単発承認が未取得 | `AwaitingApproval` |
| 許可済み送信ヘルパーへ引継ぐ | `Handoff` |
| 許可リスト違反、上限超過、検証失敗 | `Failed` |

生パケットの送信は AgentLoop や Fast Agent から直接行わない。対象許可リスト、単発トークン、実行計画ハッシュ、送信回数上限、監査ログを OS 権限分離ヘルパー側でも検証する。

## 新しい Worker を追加する手順

1. Worker の入力を型と検証規則で固定する。
2. Worker が返せる `WorkerOutcome` を定義し、任意文字列による制御を避ける。
3. `Completed` に含める事実メモから、秘密情報と生データを除く。
4. `AwaitingApproval` の場合は、Operation Plan に承認対象をハッシュ固定する。
5. Coordinator の遷移と AgentLoop の再開・キャンセル・タイムアウトをシナリオテストする。
6. Fast Agent が Worker 固有の副作用を提案しないことを確認する。

## 不変条件

1. Planner は完了を宣言できるが、最終回答の文面を所有しない。
2. Fast Agent は読み取り専用の表示担当であり、ツール実行や状態変更を行わない。
3. `AwaitingUserInput` と `AwaitingApproval` は完了へフォールスルーしない。
4. Worker の引継ぎ先は `WorkerKind` で明示し、文字列だけで任意 Worker を起動しない。
5. 完了済みまたは失敗済みの副作用を、報告処理の失敗を理由に再実行しない。

## 関連実装

- `src-tauri/src/harness/coordinator.rs`
- `src-tauri/src/harness/agent_loop.rs`
- `src-tauri/src/harness/ports.rs`
- `src-tauri/src/llm/fast_agent.rs`
- `src-tauri/src/planner/llm_planner.rs`
