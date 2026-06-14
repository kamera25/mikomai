use crate::llm::worker::LlmWorker;
use crate::llm::llm_manager::AgentContext;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::llama_backend::LlamaBackend;
use crate::llm::llm::SYSTEM_PROMPT;

const RAG_WORKER_PROMPT: &str = include_str!("../prompts/rag_worker.txt");

const MAX_NEW_TOKENS: u32 = 512;
const N_CTX: u32 = 4096;

pub struct RagWorker {
    pub ctx: Option<AgentContext<'static>>,
}

impl RagWorker {
    pub fn new(model: &LlamaModel, backend: &LlamaBackend, preload: bool) -> Result<Self, String> {
        if preload {
            let full_system_prompt = format!(
                "{}\n\n=== Current Role ===\nあなたは現在「RAG Worker (RAG回答員)」として動作しています。以下の役割指示に特化してください:\n{}",
                SYSTEM_PROMPT,
                RAG_WORKER_PROMPT
            );
            let ctx = AgentContext::new(model, backend, &full_system_prompt, 4, MAX_NEW_TOKENS, N_CTX)
                .map_err(|e| format!("Failed to create Rag context: {:?}", e))?;
            
            let ctx_static = unsafe {
                std::mem::transmute::<AgentContext<'_>, AgentContext<'static>>(ctx)
            };
            
            Ok(Self { ctx: Some(ctx_static) })
        } else {
            Ok(Self { ctx: None })
        }
    }
}

impl LlmWorker for RagWorker {
    fn agent_name(&self) -> &'static str {
        "RAG Worker (RAG回答員)"
    }

    fn context_mut(&mut self) -> &mut AgentContext<'static> {
        self.ctx.as_mut().expect("Rag context not initialized")
    }

    fn ensure_initialized(
        &mut self,
        model: &LlamaModel,
        backend: &LlamaBackend,
    ) -> Result<(), String> {
        if self.ctx.is_none() {
            let full_system_prompt = format!(
                "{}\n\n=== Current Role ===\nあなたは現在「RAG Worker (RAG回答員)」として動作しています。以下の役割指示に特化してください:\n{}",
                SYSTEM_PROMPT,
                RAG_WORKER_PROMPT
            );
            let ctx = AgentContext::new(model, backend, &full_system_prompt, 4, MAX_NEW_TOKENS, N_CTX)
                .map_err(|e| format!("Failed to create Rag context: {:?}", e))?;
            
            let ctx_static = unsafe {
                std::mem::transmute::<AgentContext<'_>, AgentContext<'static>>(ctx)
            };
            self.ctx = Some(ctx_static);
        }
        Ok(())
    }

    fn build_prompt(
        &self,
        prompt: Option<String>,
        user_message: Option<String>,
        _tool_label: Option<String>,
        output: Option<String>,
        history_block: Option<String>,
        _subsequent_task: Option<&str>,
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
