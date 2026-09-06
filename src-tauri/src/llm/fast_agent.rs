//! Fast Agent: owns concise user-facing presentation of completed work.
//!
//! It receives a completion brief from the coordinator and cannot invoke
//! tools, create plans, or change the execution flow.

use crate::llm::llm::{ask_llm_internal, LlamaState};

const FAST_AGENT_SYSTEM_PROMPT: &str = r#"You are the Fast Agent for a managed network-operations application.
Turn a completed-work brief into a concise, accurate Japanese response for the user.
Do not propose tool calls, commands to run, retries, configuration changes, or claims not supported by the brief.
If the brief says a check was only a preview, dry run, or not transmitted, state that limitation clearly.
Do not reveal secrets or reproduce raw packet payloads."#;

pub async fn present_completion(
    app: &tauri::AppHandle,
    state: &LlamaState,
    goal: &str,
    completion_brief: &str,
) -> Result<String, String> {
    // A terminal brief is expected to be short. The cap prevents a worker or
    // device output from turning the presentation role into a bulk-data path.
    let brief: String = completion_brief.chars().take(12_000).collect();
    let prompt = format!(
        "ユーザーの依頼:\n{goal}\n\n完了した処理の事実メモ:\n{brief}\n\n上記だけを根拠に、利用者へ最終報告してください。"
    );
    ask_llm_internal(&prompt, FAST_AGENT_SYSTEM_PROMPT, app, state)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn presentation_prompt_is_role_limited() {
        assert!(super::FAST_AGENT_SYSTEM_PROMPT.contains("Do not propose tool calls"));
        assert!(super::FAST_AGENT_SYSTEM_PROMPT.contains("raw packet payloads"));
    }
}
