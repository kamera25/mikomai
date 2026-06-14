use crate::llm::worker::LlmWorker;
use crate::llm::llm_manager::{ANALYSIS_WORKER_PROMPT, AgentContext};
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::llama_backend::LlamaBackend;
use crate::llm::llm::SYSTEM_PROMPT;

pub struct AnalysisWorker {
    pub ctx: AgentContext<'static>,
}

impl AnalysisWorker {
    pub fn new(model: &LlamaModel, backend: &LlamaBackend) -> Result<Self, String> {
        let full_system_prompt = format!(
            "{}\n\n=== Current Role ===\nあなたは現在「Analyst (分析官)」として動作しています。以下の役割指示に特化してください:\n{}",
            SYSTEM_PROMPT,
            ANALYSIS_WORKER_PROMPT
        );
        let ctx = AgentContext::new(model, backend, &full_system_prompt, 3, 8192)
            .map_err(|e| format!("Failed to create Analysis context: {:?}", e))?;
        
        let ctx_static = unsafe {
            std::mem::transmute::<AgentContext<'_>, AgentContext<'static>>(ctx)
        };
        
        Ok(Self { ctx: ctx_static })
    }
}

impl LlmWorker for AnalysisWorker {
    fn agent_name(&self) -> &'static str {
        "Analyst (分析官)"
    }

    fn context_mut(&mut self) -> &mut AgentContext<'static> {
        &mut self.ctx
    }

    fn build_prompt(
        &self,
        prompt: Option<String>,
        user_message: Option<String>,
        tool_label: Option<String>,
        output: Option<String>,
        history_block: Option<String>,
        subsequent_task: Option<&str>,
    ) -> String {
        if let Some(p) = prompt {
            p
        } else {
            let user_msg = user_message.as_deref().unwrap_or_default();
            let out = output.as_deref().unwrap_or_default();
            let hist = history_block.as_deref().unwrap_or_default();
            let label = tool_label.as_deref().unwrap_or_default();

            let out_formatted = if label == "Fetch Config" {
                format!(
                    "<config_data>\n{}\n</config_data>\n\n=== FINAL INSTRUCTION ===\n上記の <config_data> を分析し、ユーザーの「Subsequent Task」に対する回答を以下のフォーマットに厳密に従って日本語で出力してください。生データの出力のみは禁止します。\n\n=== Output Format & Example ===\nあなたは必ず以下のフォーマットで回答しなければなりません。生データやタグの出力のみは厳禁です。\n\n<Example_Input>\nユーザーの入力: \"NakaokuGWのDNS設定を教えて\"\nConfig: dns server 192.168.1.1\n</Example_Input>\n\n<Example_Output>\n## 結論\nNakaokuGWのDNSサーバーは、192.168.1.1 に設定されています。\n\n## 該当コンフィグ\ndns server 192.168.1.1\n\n## 分析・解説\nこの設定により、名前解決のクエリは指定されたDNSサーバーに転送されます。必要であれば疎通確認を行いますか？\n</Example_Output>\n\n## 結論\n（例: NakaokuGWのデフォルトルートはTunnel 2に設定されています。）\n\n## 該当コンフィグ\n（抽出した設定行を記載）\n\n## 分析・解説\n（なぜその設定になっているのか、関連するインターフェースやNAT、ルーティングの現在の状態などのコンテキストを運用者向けに分かりやすく解説してください。）",
                    out
                )
            } else {
                out.to_string()
            };

            let mut prompt_modified = format!(
                "ユーザーの入力: \"{}\"\nに対する{}の実行結果は以下の通りです:\n\n{}\n\n",
                user_msg, label, out_formatted
            );

            if let Some(task) = subsequent_task {
                prompt_modified.push_str(&format!(
                    "\n\n=== Subsequent Task / 後続のタスク ===\nユーザーは以下の確認・解決を望んでいます:\n{}\n必ずこの確認・解決のために必要な処理・回答を行ってください。かつ、設定の意図や現在の状態を含めて分かりやすく報告してください。",
                    task
                ));
            }

            prompt_modified.push_str(&format!(
                "\n\n # 重要! \n\n既にツールは実行済みです。この回答内で再度同じコマンド、かつ同じ引数でツール呼び出し（JSONフォーマット）を出力することは絶対に避けてください。{}\n\n【結論】",
                hist
            ));
            prompt_modified
        }
    }

    fn max_new_tokens(&self) -> u32 {
        8192
    }
}
