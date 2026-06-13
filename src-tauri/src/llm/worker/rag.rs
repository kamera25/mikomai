use crate::llm::worker::LlmWorker;
use crate::llm::llm_manager::RAG_WORKER_PROMPT;

pub struct RagWorker;

impl LlmWorker for RagWorker {
    fn agent_name(&self) -> &'static str {
        "RAG Worker (RAG回答員)"
    }

    fn system_prompt(&self, _subsequent_task: Option<&str>) -> String {
        RAG_WORKER_PROMPT.to_string()
    }

    fn build_prompt(
        &self,
        prompt: Option<String>,
        user_message: Option<String>,
        _tool_label: Option<String>,
        output: Option<String>,
        history_block: Option<String>,
    ) -> String {
        if let Some(p) = prompt {
            p
        } else {
            let user_msg = user_message.as_deref().unwrap_or_default();
            let out = output.as_deref().unwrap_or_default();
            let hist = history_block.as_deref().unwrap_or_default();
            format!(
                "ユーザーの質問: \"{}\"\nに対して、技術文書データベース(NW-DB)から以下の情報を取得しました:\n\n{}\n\nこの内容に基づき、ネットワークエンジニアの視点で、ユーザーの質問に対する적確な回答を日本語で生成してください。回答には、参照した資料の内容を具体的に含めてください。{}",
                user_msg, out, hist
            )
        }
    }
}
