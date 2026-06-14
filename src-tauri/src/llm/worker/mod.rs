pub mod router;
pub mod rag;
pub mod knowledge;
pub mod analysis;
pub mod investigate;
pub mod summarization;

pub use router::Router;
pub use rag::RagWorker;
pub use knowledge::KnowledgeWorker;
pub use analysis::AnalysisWorker;
pub use investigate::InvestigateWorker;
pub use summarization::SummarizationWorker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Investigate,
    Knowledge,
    Analysis,
    None,
}

impl Route {
    pub fn from_str(s: &str) -> Self {
        let upper = s.to_uppercase();
        if upper.contains("KNOWLEDGE") {
            Route::Knowledge
        } else if upper.contains("ANALYSIS") {
            Route::Analysis
        } else if upper.contains("NONE") {
            Route::None
        } else {
            Route::Investigate
        }
    }
}

pub trait LlmWorker {
    fn agent_name(&self) -> &'static str;
    fn context_mut(&mut self) -> &mut crate::llm::llm_manager::AgentContext<'static>;
    fn ensure_initialized(
        &mut self,
        model: &llama_cpp_2::model::LlamaModel,
        backend: &llama_cpp_2::llama_backend::LlamaBackend,
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
        model: &llama_cpp_2::model::LlamaModel,
        backend: &llama_cpp_2::llama_backend::LlamaBackend,
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
            model,
            &worker_prompt,
            window,
            temperature,
            repetition_penalty,
        ).map_err(|e| format!("Worker inference failed: {:?}", e))
    }
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
