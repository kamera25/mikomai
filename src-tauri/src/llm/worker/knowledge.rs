use crate::llm::worker::{LlmWorker, build_common_worker_prompt};
use crate::llm::llm_manager::KNOWLEDGE_WORKER_PROMPT;

pub struct KnowledgeWorker;

impl LlmWorker for KnowledgeWorker {
    fn agent_name(&self) -> &'static str {
        "Knowledge Expert (知識専門家)"
    }

    fn system_prompt(&self, subsequent_task: Option<&str>) -> String {
        let mut prompt = KNOWLEDGE_WORKER_PROMPT.to_string();
        if let Some(task) = subsequent_task {
            prompt.push_str(&format!(
                "\n\n=== Subsequent Task / 後続のタスク ===\nユーザーは以下の確認・解決を望んでいます:\n{}\n必ずこの確認・解決のために必要な処理・回答を行ってください。かつ、設定の意図や現在の状態を含めて分かりやすく報告してください。",
                task
            ));
        }
        prompt
    }

    fn build_prompt(
        &self,
        prompt: Option<String>,
        user_message: Option<String>,
        tool_label: Option<String>,
        output: Option<String>,
        history_block: Option<String>,
    ) -> String {
        build_common_worker_prompt(prompt, user_message, tool_label, output, history_block)
    }
}
