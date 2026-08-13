use crate::llm::worker::{LlmWorker, build_common_worker_prompt};
use crate::llm::llm_manager::AgentContext;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::llama_backend::LlamaBackend;
use std::sync::Arc;
use crate::llm::llm::SYSTEM_PROMPT;


const KNOWLEDGE_WORKER_PROMPT: &str = include_str!("../prompts/knowledge_worker.txt");

const MAX_NEW_TOKENS: u32 = 2048;
const N_CTX: u32 = 8192;

pub struct KnowledgeWorker {
    pub ctx: Option<AgentContext>,
    pub active_vendor: Option<String>,
    pub device_contexts: Vec<crate::llm::worker::DeviceContext>,
}

impl KnowledgeWorker {
    pub fn new(model: &Arc<LlamaModel>, backend: &Arc<LlamaBackend>, preload: bool) -> Result<Self, String> {
        if preload {
            let full_system_prompt = format!(
                "{}\n\n=== Current Role ===\nあなたは現在「Knowledge Expert (知識専門家)」として動作しています。以下の役割指示に特化してください:\n{}",
                SYSTEM_PROMPT,
                KNOWLEDGE_WORKER_PROMPT
            );
            let ctx = AgentContext::new(model.clone(), backend.clone(), &full_system_prompt, 2, MAX_NEW_TOKENS, N_CTX)
                .map_err(|e| format!("Failed to create Knowledge context: {:?}", e))?;
            
            Ok(Self { ctx: Some(ctx), active_vendor: None, device_contexts: Vec::new() })
        } else {
            Ok(Self { ctx: None, active_vendor: None, device_contexts: Vec::new() })
        }
    }

    pub fn ensure_initialized_with_vendor(
        &mut self,
        model: &Arc<LlamaModel>,
        backend: &Arc<LlamaBackend>,
        _vendor: Option<String>,
    ) -> Result<(), String> {
        let needs_init = self.ctx.is_none();

        if needs_init {
            self.ctx = None;

            let role_desc = format!(
                "あなたは現在「Knowledge Expert (知識専門家)」として動作しています。以下の役割指示に特化してください:\n{}",
                KNOWLEDGE_WORKER_PROMPT
            );
            log::debug!("=== role_desc ===\n{}", role_desc);

            let full_system_prompt = format!(
                "{}\n\n=== Current Role ===\n{}",
                SYSTEM_PROMPT,
                role_desc
            );

            let ctx = AgentContext::new(model.clone(), backend.clone(), &full_system_prompt, 2, MAX_NEW_TOKENS, N_CTX)
                .map_err(|e| format!("Failed to create Knowledge context: {:?}", e))?;
            
            self.ctx = Some(ctx);
            self.active_vendor = None;
        }
        Ok(())
    }
}

impl LlmWorker for KnowledgeWorker {
    fn agent_name(&self) -> &'static str {
        "Knowledge Expert (知識専門家)"
    }

    fn set_device_contexts(&mut self, contexts: Vec<crate::llm::worker::DeviceContext>) {
        self.device_contexts = contexts;
    }

    fn context_mut(&mut self) -> &mut AgentContext {
        self.ctx.as_mut().expect("Knowledge context not initialized")
    }

    fn ensure_initialized(
        &mut self,
        model: &Arc<LlamaModel>,
        backend: &Arc<LlamaBackend>,
    ) -> Result<(), String> {
        self.ensure_initialized_with_vendor(model, backend, None)
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
        // Connection details are now provided in device_contexts
        let matched_context = self.device_contexts.first().cloned();

        self.ensure_initialized_with_vendor(model, backend, None)?;

        let (prompt, user_message, output, history_block) = crate::llm::llm_manager::apply_token_budget(
            model,
            self.context_mut().n_ctx,
            self.context_mut().base_n_past,
            self.context_mut().max_new_tokens,
            prompt,
            user_message,
            output,
            history_block,
        ).map_err(|e| format!("Failed to apply token budget: {:?}", e))?;

        let worker_prompt = self.build_prompt(
            prompt,
            user_message,
            tool_label,
            output,
            history_block,
            subsequent_task,
        );

        let (vendor_str, device_str, gateway_str) = if let Some(ctx) = &matched_context {
            let v = if ctx.vendor.is_empty() { "Unknown".to_string() } else { ctx.vendor.clone() };
            let d = if ctx.device_type.is_empty() { "Unknown".to_string() } else { ctx.device_type.clone() };
            let g = format!("{} ({})", ctx.hostname, ctx.ip);
            (v, d, g)
        } else {
            ("Unknown".to_string(), "Unknown".to_string(), "Unknown".to_string())
        };

        let final_prompt = format!(
            "### System Metadata\n\
             ユーザーのクエリを処理するにあたり、以下の前提条件（対象環境）を考慮してください。\n\
             - Vendor: {}\n\
             - Device: {}\n\
             - Gateway Name (IP): {}\n\n\
             ### User Query\n\
             User: {}\n\
             MIKOMAI:",
            vendor_str,
            device_str,
            gateway_str,
            worker_prompt
        );

        crate::llm::llm_manager::run_inference(
            self.context_mut(),
            &final_prompt,
            window,
            temperature,
            repetition_penalty,
        ).map_err(|e| format!("Worker inference failed: {:?}", e))
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
