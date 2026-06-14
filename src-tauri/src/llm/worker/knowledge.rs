use crate::llm::worker::{LlmWorker, build_common_worker_prompt};
use crate::llm::llm_manager::AgentContext;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::llama_backend::LlamaBackend;
use crate::llm::llm::SYSTEM_PROMPT;

const KNOWLEDGE_WORKER_PROMPT: &str = include_str!("../prompts/knowledge_worker.txt");

const MAX_NEW_TOKENS: u32 = 2048;
const N_CTX: u32 = 4096;

pub struct KnowledgeWorker {
    pub ctx: Option<AgentContext<'static>>,
}

impl KnowledgeWorker {
    pub fn new(model: &LlamaModel, backend: &LlamaBackend, preload: bool) -> Result<Self, String> {
        if preload {
            let full_system_prompt = format!(
                "{}\n\n=== Current Role ===\nあなたは現在「Knowledge Expert (知識専門家)」として動作しています。以下の役割指示に特化してください:\n{}",
                SYSTEM_PROMPT,
                KNOWLEDGE_WORKER_PROMPT
            );
            let ctx = AgentContext::new(model, backend, &full_system_prompt, 2, MAX_NEW_TOKENS, N_CTX)
                .map_err(|e| format!("Failed to create Knowledge context: {:?}", e))?;
            
            let ctx_static = unsafe {
                std::mem::transmute::<AgentContext<'_>, AgentContext<'static>>(ctx)
            };
            
            Ok(Self { ctx: Some(ctx_static) })
        } else {
            Ok(Self { ctx: None })
        }
    }
}

impl LlmWorker for KnowledgeWorker {
    fn agent_name(&self) -> &'static str {
        "Knowledge Expert (知識専門家)"
    }

    fn context_mut(&mut self) -> &mut AgentContext<'static> {
        self.ctx.as_mut().expect("Knowledge context not initialized")
    }

    fn ensure_initialized(
        &mut self,
        model: &LlamaModel,
        backend: &LlamaBackend,
    ) -> Result<(), String> {
        if self.ctx.is_none() {
            let full_system_prompt = format!(
                "{}\n\n=== Current Role ===\nあなたは現在「Knowledge Expert (知識専門家)」として動作しています。以下の役割指示に特化してください:\n{}",
                SYSTEM_PROMPT,
                KNOWLEDGE_WORKER_PROMPT
            );
            let ctx = AgentContext::new(model, backend, &full_system_prompt, 2, MAX_NEW_TOKENS, N_CTX)
                .map_err(|e| format!("Failed to create Knowledge context: {:?}", e))?;
            
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
        tool_label: Option<String>,
        output: Option<String>,
        history_block: Option<String>,
        subsequent_task: Option<&str>,
    ) -> String {
        build_common_worker_prompt(prompt, user_message, tool_label, output, history_block, subsequent_task)
    }
}
