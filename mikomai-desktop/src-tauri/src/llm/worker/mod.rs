pub use crate::llm::router::llm_router::Router;
pub use device_context::{format_device_contexts, resolve_device_contexts, DeviceContext};
pub mod analysis;
pub mod builder;
pub mod device_context;
pub mod knowledge;
pub mod plotter;
pub mod rag;
pub mod summarization;
pub use analysis::AnalysisWorker;
pub use builder::BuilderWorker;
pub use knowledge::KnowledgeWorker;
pub use plotter::PlotterWorker;
pub use rag::RagWorker;
pub use summarization::SummarizationWorker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Agent,
    Knowledge,
    Analysis,
    Plotter,
    Builder,
    None,
}

impl std::str::FromStr for Route {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let upper = s.to_uppercase();
        let route = if upper.contains("AGENT") || upper.contains("INVESTIGATE") {
            // INVESTIGATE is accepted for router-output compatibility with
            // older persisted prompts, but AGENT is the canonical route.
            Route::Agent
        } else if upper.contains("KNOWLEDGE") {
            Route::Knowledge
        } else if upper.contains("ANALYSIS") {
            Route::Analysis
        } else if upper.contains("PLOTTER") || upper.contains("PLOTER") {
            Route::Plotter
        } else if upper.contains("BUILDER") {
            Route::Builder
        } else if upper.contains("NONE") {
            Route::None
        } else {
            Route::Agent
        };
        Ok(route)
    }
}

pub trait LlmWorker {
    fn agent_name(&self) -> &'static str;
    fn context_mut(&mut self) -> &mut crate::llm::llm_manager::AgentContext;
    fn ensure_initialized(
        &mut self,
        model: &std::sync::Arc<llama_cpp_2::model::LlamaModel>,
        backend: &std::sync::Arc<llama_cpp_2::llama_backend::LlamaBackend>,
    ) -> Result<(), String>;
    fn build_prompt(
        &self,
        prompt: Option<String>,
        user_message: Option<String>,
        tool_label: Option<String>,
        output: Option<String>,
        history_block: Option<String>,
        subsequent_task: Option<&str>,
    ) -> String;
    #[allow(dead_code)]
    fn max_new_tokens(&self) -> u32 {
        2048
    }

    fn ask(
        &mut self,
        model: &std::sync::Arc<llama_cpp_2::model::LlamaModel>,
        backend: &std::sync::Arc<llama_cpp_2::llama_backend::LlamaBackend>,
        prompt: Option<String>,
        user_message: Option<String>,
        tool_label: Option<String>,
        output: Option<String>,
        history_block: Option<String>,
        subsequent_task: Option<&str>,
        window: Option<&tauri::Window>,
        temperature: f32,
        repetition_penalty: f32,
    ) -> Result<String, String> {
        self.ensure_initialized(model, backend)?;
        let (prompt, user_message, output, history_block) =
            crate::llm::llm_manager::apply_token_budget(
                model,
                self.context_mut().n_ctx,
                self.context_mut().base_n_past,
                self.context_mut().max_new_tokens,
                prompt,
                user_message,
                output,
                history_block,
            )
            .map_err(|e| format!("Failed to apply token budget: {:?}", e))?;

        let worker_prompt = self.build_prompt(
            prompt,
            user_message,
            tool_label,
            output,
            history_block,
            subsequent_task,
        );
        crate::llm::llm_manager::run_inference(
            self.context_mut(),
            &worker_prompt,
            window,
            temperature,
            repetition_penalty,
        )
        .map_err(|e| format!("Worker inference failed: {:?}", e))
    }

    fn set_device_contexts(&mut self, _contexts: Vec<crate::llm::worker::DeviceContext>) {}
}

pub fn build_common_worker_prompt(
    prompt: Option<String>,
    user_message: Option<String>,
    tool_label: Option<String>,
    output: Option<String>,
    history_block: Option<String>,
    subsequent_task: Option<&str>,
) -> String {
    let mut base = if let Some(p) = prompt {
        p
    } else {
        let user_msg = user_message.as_deref().unwrap_or_default();
        let out = output.as_deref().unwrap_or_default();
        let hist = history_block.as_deref().unwrap_or_default();
        let label = tool_label.as_deref().unwrap_or_default();

        let out_formatted = out.to_string();

        let mut prompt_modified = format!(
            "ユーザーの入力: \"{}\"\nに対する{}の実行結果は以下の通りです:\n\n{}\n\n",
            user_msg, label, out_formatted
        );

        prompt_modified.push_str(&format!(
            "\n\n # 重要! \n\n既にツールは実行済みです。この回答内で再度同じコマンド、かつ同じ引数でツール呼び出し（JSONフォーマット）を出力することは絶対に避けてください。{}",
            hist
        ));
        prompt_modified
    };

    if let Some(task) = subsequent_task {
        base.push_str(&format!(
            "\n\n=== Subsequent Task / 後続のタスク ===\nユーザーは以下の確認・解決を望んでいます:\n{}\n必ずこの確認・解決のために必要な処理・回答を行ってください。かつ、設定の意図や現在の状態を含めて分かりやすく報告してください。",
            task
        ));
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_common_worker_prompt_default() {
        let prompt = build_common_worker_prompt(
            None,
            Some("Check interfaces".to_string()),
            Some("Show Interfaces".to_string()),
            Some("GigabitEthernet0/1 is up".to_string()),
            Some("\n\n<memory>\n1. memory content\n</memory>".to_string()),
            Some("Fix interface duplex settings"),
        );

        assert!(prompt.contains("Check interfaces"));
        assert!(prompt.contains("Show Interfaces"));
        assert!(prompt.contains("GigabitEthernet0/1 is up"));
        assert!(prompt.contains("memory content"));
        assert!(prompt.contains("Fix interface duplex settings"));
        assert!(prompt.contains("既にツールは実行済みです"));
    }

    #[test]
    fn test_build_common_worker_prompt_override() {
        let prompt = build_common_worker_prompt(
            Some("Custom overriding prompt".to_string()),
            None,
            None,
            None,
            None,
            Some("subsequent task"),
        );

        assert!(prompt.starts_with("Custom overriding prompt"));
        assert!(prompt.contains("subsequent task"));
    }
}
