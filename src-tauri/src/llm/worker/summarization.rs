use crate::llm::llm_manager::AgentContext;
use crate::llm::worker::LlmWorker;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::LlamaModel;
use std::sync::Arc;

const SUMMARIZATION_PROMPT: &str = include_str!("../prompts/summarization_prompt.txt");

const MAX_NEW_TOKENS: u32 = 256;
const N_CTX: u32 = 8192;

pub struct SummarizationWorker
{
    pub ctx: Option<AgentContext>,
}

impl SummarizationWorker
{
    pub fn new(
        model: &Arc<LlamaModel>,
        backend: &Arc<LlamaBackend>,
        preload: bool,
    ) -> Result<Self, String>
    {
        if preload
        {
            let ctx = AgentContext::new(
                model.clone(),
                backend.clone(),
                SUMMARIZATION_PROMPT,
                5,
                MAX_NEW_TOKENS,
                N_CTX,
            )
            .map_err(|e| format!("Failed to create Summarization context: {:?}", e))?;
            Ok(Self { ctx: Some(ctx) })
        }
        else
        {
            Ok(Self { ctx: None })
        }
    }
}

impl LlmWorker for SummarizationWorker
{
    fn agent_name(&self) -> &'static str
    {
        "Summarization Unit (要約ユニット)"
    }

    fn context_mut(&mut self) -> &mut AgentContext
    {
        self.ctx
            .as_mut()
            .expect("Summarization context not initialized")
    }

    fn ensure_initialized(
        &mut self,
        model: &Arc<LlamaModel>,
        backend: &Arc<LlamaBackend>,
    ) -> Result<(), String>
    {
        if self.ctx.is_none()
        {
            let ctx = AgentContext::new(
                model.clone(),
                backend.clone(),
                SUMMARIZATION_PROMPT,
                5,
                MAX_NEW_TOKENS,
                N_CTX,
            )
            .map_err(|e| format!("Failed to create Summarization context: {:?}", e))?;
            self.ctx = Some(ctx);
        }
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
    ) -> String
    {
        prompt.unwrap_or_default()
    }
}
