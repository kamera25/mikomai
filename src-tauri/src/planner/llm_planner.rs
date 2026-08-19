use crate::llm::llm::LlamaState;
use crate::planner::decision::parse_decision_from_json;
use crate::state::events::Decision;
use crate::state::network_state::NetworkState;
use tauri::AppHandle;

const PLANNER_SYSTEM_PROMPT: &str = r#"あなたは Network Agent Harness の中核を担う LLM Planner です。
与えられた Network State（観察された事実、検証中の仮説、目標）をもとに、目標達成のために次に実行すべきアクション（Decision）を構造化されたJSONフォーマットで提案してください。

【制約事項】
1. 事実（Observed）と推測（Hypothesis）を明確に区別すること。
2. アクションスペースは以下のいずれかから選択すること:
   - "OBSERVE": 調査コマンドやPing/Traceroute等の読み取り系Toolを実行して状態を確認
   - "VERIFY": 設定変更後や問題解消後の検証確認
   - "CONFIGURE": 機器への設定適用
   - "ROLLBACK": 問題発生時の切り戻し
   - "ASK_HUMAN": ユーザーに追加情報や確認を求める
   - "FINISH": 目標が完全に達成され、調査や作業が完了した
3. 必ず以下のJSON構造のみを出力してください（Markdownコードブロック```json ... ```で囲むこと）。

```json
{
  "action_type": "OBSERVE" | "VERIFY" | "CONFIGURE" | "ROLLBACK" | "ASK_HUMAN" | "FINISH",
  "objective": "このアクションの具体的な目的",
  "tool": "利用するツール名 (例: network_show, self_network_ping, fetch_config 等。FINISHの場合はnull)",
  "target": "対象機器名またはホスト名",
  "parameters": {
    "command": "show ip route 等のパラメータ"
  },
  "reason": [
    "アクションを選択した理由1",
    "理由2"
  ],
  "expected_observation": [
    "このアクションで期待される観察結果"
  ],
  "final_answer": "FINISHの場合にユーザーへ提示する最終報告サマリー"
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
