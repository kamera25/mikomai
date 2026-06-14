use crate::llm::worker::LlmWorker;
use crate::llm::llm_manager::AgentContext;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::llama_backend::LlamaBackend;

const SUMMARIZATION_PROMPT: &str = include_str!("../prompts/summarization_prompt.txt");

const MAX_NEW_TOKENS: u32 = 256;
const N_CTX: u32 = 4096;

pub struct SummarizationWorker {
    pub ctx: AgentContext<'static>,
}

impl SummarizationWorker {
    pub fn new(model: &LlamaModel, backend: &LlamaBackend) -> Result<Self, String> {
        let ctx = AgentContext::new(model, backend, SUMMARIZATION_PROMPT, 5, MAX_NEW_TOKENS, N_CTX)
            .map_err(|e| format!("Failed to create Summarization context: {:?}", e))?;
        
        let ctx_static = unsafe {
            std::mem::transmute::<AgentContext<'_>, AgentContext<'static>>(ctx)
        };
        
        Ok(Self { ctx: ctx_static })
    }
}

impl LlmWorker for SummarizationWorker {
    fn agent_name(&self) -> &'static str {
        "Summarization Unit (要約ユニット)"
    }

    fn context_mut(&mut self) -> &mut AgentContext<'static> {
        &mut self.ctx
    }

    fn ensure_initialized(
        &mut self,
        _model: &LlamaModel,
        _backend: &LlamaBackend,
    ) -> Result<(), String> {
        Ok(())
    }

    fn build_prompt(
        &self,
        prompt: Option<String>,
        _user_message: Option<String>,
        _tool_label: Option<String>,
        _output: Option<String>,
        _history_block: Option<String>,
        _subsequent_task: Option<&str>,
    ) -> String {
        prompt.unwrap_or_default()
    }
}
