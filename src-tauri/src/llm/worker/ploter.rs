use crate::llm::worker::LlmWorker;
use crate::llm::llm_manager::AgentContext;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::llama_backend::LlamaBackend;
use std::sync::Arc;
use crate::llm::llm::SYSTEM_PROMPT;

const PLOTER_WORKER_PROMPT: &str = include_str!("../prompts/ploter_worker.txt");

const MAX_NEW_TOKENS: u32 = 2048;
const N_CTX: u32 = 4096;

pub struct PloterWorker {
    pub ctx: Option<AgentContext>,
}

impl PloterWorker {
    pub fn new(model: &Arc<LlamaModel>, backend: &Arc<LlamaBackend>, preload: bool) -> Result<Self, String> {
        if preload {
            let full_system_prompt = format!(
                "{}\n\n=== Current Role ===\nあなたは現在「Ploter (作図器)」として動作しています。以下の役割指示に特化してください:\n{}",
                SYSTEM_PROMPT,
                PLOTER_WORKER_PROMPT
            );
            let ctx = AgentContext::new(model.clone(), backend.clone(), &full_system_prompt, 6, MAX_NEW_TOKENS, N_CTX)
                .map_err(|e| format!("Failed to create Ploter context: {:?}", e))?;
            
            Ok(Self { ctx: Some(ctx) })
        } else {
            Ok(Self { ctx: None })
        }
    }
}

impl LlmWorker for PloterWorker {
    fn agent_name(&self) -> &'static str {
        "Ploter (作図器)"
    }

    fn context_mut(&mut self) -> &mut AgentContext {
        self.ctx.as_mut().expect("Ploter context not initialized")
    }

    fn ensure_initialized(
        &mut self,
        model: &Arc<LlamaModel>,
        backend: &Arc<LlamaBackend>,
    ) -> Result<(), String> {
        if self.ctx.is_none() {
            let full_system_prompt = format!(
                "{}\n\n=== Current Role ===\nあなたは現在「Ploter (作図器)」として動作しています。以下の役割指示に特化してください:\n{}",
                SYSTEM_PROMPT,
                PLOTER_WORKER_PROMPT
            );
            let ctx = AgentContext::new(model.clone(), backend.clone(), &full_system_prompt, 6, MAX_NEW_TOKENS, N_CTX)
                .map_err(|e| format!("Failed to create Ploter context: {:?}", e))?;
            
            self.ctx = Some(ctx);
        }
        Ok(())
    }

    fn max_new_tokens(&self) -> u32 {
        MAX_NEW_TOKENS
    }

    fn ask(
        &mut self,
        model: &Arc<LlamaModel>,
        backend: &Arc<LlamaBackend>,
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

        let schema = r#"{
            "type": "object",
            "properties": {
                "tool_name": { "type": "string", "enum": ["self_network_nwdiag"] },
                "params": {
                    "type": "object",
                    "properties": {
                        "schema": { "type": "string" }
                    },
                    "required": ["schema"]
                }
            },
            "required": ["tool_name", "params"]
        }"#;

        let grammar_str = llama_cpp_2::json_schema_to_grammar(schema)
            .map_err(|e| format!("Failed to convert schema to grammar: {:?}", e))?;

        let grammar_sampler = llama_cpp_2::sampling::LlamaSampler::grammar(&self.context_mut().model, &grammar_str, "root")
            .map_err(|e| format!("Failed to create grammar sampler: {:?}", e))?;

        crate::llm::llm_manager::run_inference_with_grammar(
            self.context_mut(),
            &worker_prompt,
            window,
            temperature,
            repetition_penalty,
            Some(grammar_sampler),
        ).map_err(|e| format!("Ploter inference failed: {:?}", e))
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
        crate::llm::worker::build_common_worker_prompt(prompt, user_message, tool_label, output, history_block, subsequent_task)
    }
}
