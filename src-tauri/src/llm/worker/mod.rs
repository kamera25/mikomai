pub mod rag;
pub mod knowledge;
pub mod analysis;
pub mod investigate;

pub use rag::RagWorker;
pub use knowledge::KnowledgeWorker;
pub use analysis::AnalysisWorker;
pub use investigate::InvestigateWorker;

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
    fn system_prompt(&self, subsequent_task: Option<&str>) -> String;
    fn build_prompt(
        &self,
        prompt: Option<String>,
        user_message: Option<String>,
        tool_label: Option<String>,
        output: Option<String>,
        history_block: Option<String>,
    ) -> String;
}

pub fn build_common_worker_prompt(
    prompt: Option<String>,
    user_message: Option<String>,
    tool_label: Option<String>,
    output: Option<String>,
    history_block: Option<String>,
) -> String {
    if let Some(p) = prompt {
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
    }
}
