use crate::llm::worker::{LlmWorker, build_common_worker_prompt};
use crate::llm::llm_manager::{INVESTIGATE_WORKER_PROMPT, AgentContext};
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::llama_backend::LlamaBackend;
use crate::llm::llm::SYSTEM_PROMPT;

pub struct InvestigateWorker {
    pub ctx: AgentContext<'static>,
}

impl InvestigateWorker {
    pub fn new(model: &LlamaModel, backend: &LlamaBackend) -> Result<Self, String> {
        let full_system_prompt = format!(
            "{}\n\n=== Current Role ===\nあなたは現在「Investigator (調査員)」として動作しています。以下の役割指示に特化してください:\n{}",
            SYSTEM_PROMPT,
            INVESTIGATE_WORKER_PROMPT
        );
        let ctx = AgentContext::new(model, backend, &full_system_prompt, 1, 2048)
            .map_err(|e| format!("Failed to create Investigate context: {:?}", e))?;
        
        let ctx_static = unsafe {
            std::mem::transmute::<AgentContext<'_>, AgentContext<'static>>(ctx)
        };
        
        Ok(Self { ctx: ctx_static })
    }
}

impl LlmWorker for InvestigateWorker {
    fn agent_name(&self) -> &'static str {
        "Investigator (調査員)"
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
        build_common_worker_prompt(prompt, user_message, tool_label, output, history_block, subsequent_task)
    }
}
