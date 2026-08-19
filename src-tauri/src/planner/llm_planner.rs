use crate::llm::llm::LlamaState;
use crate::planner::decision::parse_decision_from_json;
use crate::state::events::Decision;
use crate::state::network_state::NetworkState;
use tauri::AppHandle;

const PLANNER_SYSTEM_PROMPT: &str = r#"あなたは Network Agent Harness の中核を担う LLM Planner です。
与えられた Network State（これまでに実行したツールとその結果、観察された事実、目標）をもとに、目標達成のために次に実行すべきアクション（Decision）を構造化されたJSONフォーマットで提案してください。

【重要な行動指針】
1. 「これまでに実行したツールとその結果」を必ず確認してください。
2. 既にツールを実行し結果が得られている場合（例: Pingが成功した、またはエラーが返った、showコマンド結果が取得できたなど）：
   - その結果をもってユーザーの目標（質問や調査）に回答できる場合は、再度ツールを実行せず、直ちに action_type: "FINISH" を選択し、final_answer に分かりやすい分析・結果サマリーを記述してください。
   - 同じツール・同じ引数の再実行（無限ループ）は絶対に避けてください。
3. アクションスペースは以下のいずれかを選択すること:
   - "OBSERVE": 調査コマンドやPing/Traceroute等の読み取り系Toolを実行して状態を確認
   - "VERIFY": 設定変更後や問題解消後の検証確認
   - "CONFIGURE": 機器への設定適用
   - "ROLLBACK": 問題発生時の切り戻し
   - "ASK_HUMAN": ユーザーに追加情報や確認を求める
   - "FINISH": 目標が達成され、調査・作業が完了した（final_answerにユーザーへの最終回答を記述）
4. 主な利用可能ツールと引数例:
   - self_network_ping: {"host": "192.168.1.1"}
   - self_network_traceroute: {"host": "192.168.1.1"}
   - network_show: {"command": "show ip route"} (targetに対象機器名を指定)
   - self_network_route: {}
   - self_network_arp: {}
5. 必ず以下のJSON構造のみを出力してください（Markdownコードブロック```json ... ```で囲むこと）。

```json
{
  "action_type": "OBSERVE" | "VERIFY" | "CONFIGURE" | "ROLLBACK" | "ASK_HUMAN" | "FINISH",
  "objective": "このアクションの具体的な目的",
  "tool": "利用するツール名 (FINISHの場合はnull)",
  "target": "対象機器名またはホスト名 (不要な場合はnull)",
  "parameters": {
    "host": "Ping等の対象ホスト",
    "command": "showコマンド等の文字列"
  },
  "reason": [
    "アクションを選択した理由や、前回のツール実行結果に対する評価"
  ],
  "expected_observation": [
    "このアクションで期待される観察結果"
  ],
  "final_answer": "FINISHの場合にユーザーへ提示する最終報告サマリー（Markdown形式）"
}
```
"#;


pub struct LlmPlanner;

impl LlmPlanner {
    pub async fn plan(
        app: &AppHandle,
        llama_state: &LlamaState,
        network_state: &NetworkState,
    ) -> Result<Decision, String> {
        let state_prompt = network_state.to_prompt_context();
        let full_prompt = format!(
            "{}\n\n上記の状態を踏まえ、目標を達成するために次に行うべき最善の Decision をJSONで出力してください。",
            state_prompt
        );

        let response = crate::llm::llm::ask_llm_internal(
            &full_prompt,
            PLANNER_SYSTEM_PROMPT,
            app,
            llama_state,
        )
        .await
        .map_err(|e| format!("Planner inference failed: {}", e))?;

        log::info!("LLM Planner Raw Response:\n{}", response);

        parse_decision_from_json(&response)
    }
}
