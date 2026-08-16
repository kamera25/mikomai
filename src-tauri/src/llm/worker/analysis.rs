use crate::llm::llm::SYSTEM_PROMPT;
use crate::llm::llm_manager::AgentContext;
use crate::llm::worker::LlmWorker;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::LlamaModel;
use std::sync::Arc;

const ANALYSIS_WORKER_PROMPT: &str = include_str!("../prompts/analysis_worker.txt");

const MAX_NEW_TOKENS: u32 = 2048;
const N_CTX: u32 = 8740;

pub struct AnalysisWorker
{
    pub ctx: Option<AgentContext>,
}

impl AnalysisWorker
{
    pub fn new(
        model: &Arc<LlamaModel>,
        backend: &Arc<LlamaBackend>,
        preload: bool,
    ) -> Result<Self, String>
    {
        if preload
        {
            let full_system_prompt = format!(
                "{}\n\n=== Current Role ===\nあなたは現在「Analyst (分析官)」として動作しています。以下の役割指示に特化してください:\n{}",
                SYSTEM_PROMPT,
                ANALYSIS_WORKER_PROMPT
            );
            let ctx = AgentContext::new(
                model.clone(),
                backend.clone(),
                &full_system_prompt,
                3,
                MAX_NEW_TOKENS,
                N_CTX,
            )
            .map_err(|e| format!("Failed to create Analysis context: {:?}", e))?;

            Ok(Self { ctx: Some(ctx) })
        }
        else
        {
            Ok(Self { ctx: None })
        }
    }
}

impl LlmWorker for AnalysisWorker
{
    fn agent_name(&self) -> &'static str
    {
        "Analyst (分析官)"
    }

    fn context_mut(&mut self) -> &mut AgentContext
    {
        self.ctx.as_mut().expect("Analysis context not initialized")
    }

    fn ensure_initialized(
        &mut self,
        model: &Arc<LlamaModel>,
        backend: &Arc<LlamaBackend>,
    ) -> Result<(), String>
    {
        if self.ctx.is_none()
        {
            let full_system_prompt = format!(
                "{}\n\n=== Current Role ===\nあなたは現在「Analyst (分析官)」として動作しています。以下の役割指示に特化してください:\n{}",
                SYSTEM_PROMPT,
                ANALYSIS_WORKER_PROMPT
            );
            let ctx = AgentContext::new(
                model.clone(),
                backend.clone(),
                &full_system_prompt,
                3,
                MAX_NEW_TOKENS,
                N_CTX,
            )
            .map_err(|e| format!("Failed to create Analysis context: {:?}", e))?;

            self.ctx = Some(ctx);
        }
        Ok(())
    }

    fn build_prompt(
        &self,
        prompt: Option<String>,
        user_message: Option<String>,
        tool_label: Option<String>,
        output: Option<String>,
        history_block: Option<String>,
        subsequent_task: Option<&str>,
    ) -> String
    {
        if let Some(p) = prompt
        {
            p
        }
        else
        {
            let user_msg = user_message.as_deref().unwrap_or_default();
            let out = output.as_deref().unwrap_or_default();
            let hist = history_block.as_deref().unwrap_or_default();
            let label = tool_label.as_deref().unwrap_or_default();

            let out_formatted = if label == "Fetch Config"
            {
                format!(
                    "<config_data>\n{}\n</config_data>\n\n=== FINAL INSTRUCTION ===\n上記の <config_data> を分析し、ユーザーの「Subsequent Task」に対する回答を以下のフォーマットに厳密に従って日本語で出力してください。生データの出力のみは禁止します。\n\n=== Output Format & Example ===\nあなたは必ず以下のフォーマットで回答しなければなりません。生データやタグの出力のみは厳禁です。\n\n<Example_Input>\nユーザーの入力: \"NakaokuGWのDNS設定を教えて\"\nConfig: dns server 192.168.1.1\n</Example_Input>\n\n<Example_Output>\n## 結論\nNakaokuGWのDNSサーバーは、192.168.1.1 に設定されています。\n\n## 該当コンフィグ\ndns server 192.168.1.1\n\n## 分析・解説\nこの設定により、名前解決のクエリは指定されたDNSサーバーに転送されます。必要であれば疎通確認を行いますか？\n</Example_Output>\n\n## 結論\n（例: NakaokuGWのデフォルトルートはTunnel 2に設定されています。）\n\n## 該当コンフィグ\n（抽出した設定行を記載）\n\n## 分析・解説\n（なぜその設定になっているのか、関連するインターフェースやNAT、ルーティングの現在の状態などのコンテキストを運用者向けに分かりやすく解説してください。）",
                    out
                )
            }
            else
            {
                out.to_string()
            };

            let mut prompt_modified = format!(
                "ユーザーの入力: \"{}\"\nに対する{}の実行結果は以下の通りです:\n\n{}\n\n",
                user_msg, label, out_formatted
            );

            if let Some(task) = subsequent_task
            {
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

    fn max_new_tokens(&self) -> u32
    {
        8192
    }
}
