// LLM Manager for worker context and prompts
use anyhow::Result;
use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::model::AddBos;
use std::sync::Arc;
use tauri::Emitter;
pub struct SharedWorkers {
    pub router: std::sync::Mutex<crate::llm::worker::Router>,
    pub investigate: std::sync::Mutex<crate::llm::worker::InvestigateWorker>,
    pub knowledge: std::sync::Mutex<crate::llm::worker::KnowledgeWorker>,
    pub analysis: std::sync::Mutex<crate::llm::worker::AnalysisWorker>,
    pub rag: std::sync::Mutex<crate::llm::worker::RagWorker>,
    pub summarization: std::sync::Mutex<crate::llm::worker::SummarizationWorker>,
    pub ploter: std::sync::Mutex<crate::llm::worker::PloterWorker>,
}

pub struct SharedModel {
    pub workers: Option<SharedWorkers>,
    pub model: Arc<LlamaModel>,
    pub backend: Arc<LlamaBackend>,
}

impl Drop for SharedModel {
    fn drop(&mut self) {
        // Explicitly drop workers first so that they drop their contexts which borrow the model/backend
        self.workers.take();
    }
}

impl std::ops::Deref for SharedModel {
    type Target = SharedWorkers;
    fn deref(&self) -> &Self::Target {
        self.workers.as_ref().expect("Workers not initialized or already dropped")
    }
}

pub struct AgentContext {
    // IMPORTANT: Field ordering matters for drop safety.
    // `ctx` MUST be declared before `_backend` and `model` so that it is dropped first.
    // This guarantees the LlamaContext (which borrows from model/backend) is destroyed
    // before the Arc references that keep the underlying data alive.
    pub ctx: LlamaContext<'static>,
    pub model: Arc<LlamaModel>,
    _backend: Arc<LlamaBackend>,
    pub base_n_past: u32,
    pub id: i32,
    pub n_ctx: u32,
    pub max_new_tokens: u32,
}

// SAFETY: AgentContext is only accessed through std::sync::Mutex in SharedWorkers,
// ensuring exclusive access. LlamaContext itself is not Send/Sync, but our usage
// pattern (single-threaded inference worker with Mutex protection) makes this safe.
unsafe impl Send for AgentContext {}
unsafe impl Sync for AgentContext {}

impl AgentContext {
    pub fn new(
        model: Arc<LlamaModel>,
        backend: Arc<LlamaBackend>,
        system_prompt: &str,
        id: i32,
        max_new_tokens: u32,
        n_ctx: u32,
    ) -> Result<Self> {
        let formatted_sys = format!("<|turn>system\n{}<turn|>\n", system_prompt);
        let mut tokens = model.str_to_token(&formatted_sys, AddBos::Always)?;

        // Ensure system prompt doesn't exceed 8192 tokens to leave room for user query + generation
        let max_sys_tokens = 8192;
        if tokens.len() > max_sys_tokens {
            tokens.truncate(max_sys_tokens);
        }

        let tokens_len = tokens.len();

        let mut ctx_params = LlamaContextParams::default();
        ctx_params = ctx_params.with_n_ctx(std::num::NonZeroU32::new(n_ctx));
        ctx_params = ctx_params.with_n_batch(n_ctx);
        ctx_params = ctx_params.with_type_k(llama_cpp_2::context::params::KvCacheType::Q4_0);
        ctx_params = ctx_params.with_type_v(llama_cpp_2::context::params::KvCacheType::Q4_0);
        ctx_params = ctx_params.with_flash_attention_policy(1);

        // SAFETY: We obtain a &'static reference from the Arc pointer. This is safe because:
        // 1. The Arc<LlamaBackend> is stored in `self._backend`, keeping the data alive.
        // 2. The `ctx` field is declared before `_backend` in the struct, so Rust's drop order
        //    guarantees the LlamaContext is dropped before the Arc<LlamaBackend>.
        // 3. Therefore, the backend reference remains valid for the entire lifetime of `ctx`.
        let backend_ref: &'static LlamaBackend = unsafe { &*Arc::as_ptr(&backend) };
        let model_ref: &'static LlamaModel = unsafe { &*Arc::as_ptr(&model) };

        let mut ctx = model_ref.new_context(backend_ref, ctx_params)?;

        let mut batch = LlamaBatch::new(n_ctx as usize, 1);
        let last_index = tokens_len - 1;

        for (i, token) in tokens.into_iter().enumerate() {
            let is_last = i == last_index;
            batch.add(token, i as i32, &[0], is_last)?;
        }

        ctx.decode(&mut batch)?;

        let base_n_past = tokens_len as u32;

        Ok(Self {
            ctx,
            model,
            _backend: backend,
            base_n_past,
            id,
            n_ctx,
            max_new_tokens,
        })
    }
}

fn process_token_bytes(
    bytes_accumulator: &mut Vec<u8>,
    result_string: &mut String,
    window: Option<&tauri::Window>,
) {
    match std::str::from_utf8(bytes_accumulator) {
        Ok(s) => {
            if let Some(w) = window {
                let _ = w.emit("chat-event", crate::mcp::protocol::ChatEvent::LlmChunk(s.to_string()));
            }
            result_string.push_str(s);
            bytes_accumulator.clear();
        }
        Err(e) => {
            let utf8_error_index = e.valid_up_to();
            let valid_str = String::from_utf8_lossy(&bytes_accumulator[..utf8_error_index]).to_string();
            if let Some(w) = window {
                let _ = w.emit("chat-event", crate::mcp::protocol::ChatEvent::LlmChunk(valid_str.clone()));
            }
            result_string.push_str(&valid_str);
            bytes_accumulator.drain(..utf8_error_index);
            if bytes_accumulator.len() > 8 {
                 let s = String::from_utf8_lossy(bytes_accumulator);
                 if let Some(w) = window {
                     let _ = w.emit("chat-event", crate::mcp::protocol::ChatEvent::LlmChunk(s.to_string()));
                 }
                 result_string.push_str(&s);
                 bytes_accumulator.clear();
            }
        }
    }
}

pub fn run_inference(
    agent_ctx: &mut AgentContext,
    prompt: &str,
    window: Option<&tauri::Window>,
    temperature: f32,
    repetition_penalty: f32,
) -> Result<String> {
    run_inference_with_grammar(agent_ctx, prompt, window, temperature, repetition_penalty, None)
}

struct KVCacheGuard<'a> {
    ctx: &'a mut LlamaContext<'static>,
    base_n_past: u32,
}

impl<'a> Drop for KVCacheGuard<'a> {
    fn drop(&mut self) {
        if let Err(e) = self.ctx.clear_kv_cache_seq(Some(0), Some(self.base_n_past), None) {
            log::error!("Failed to clear KV cache in Drop guard: {:?}", e);
        }
    }
}

pub fn run_inference_with_grammar(
    agent_ctx: &mut AgentContext,
    prompt: &str,
    window: Option<&tauri::Window>,
    temperature: f32,
    repetition_penalty: f32,
    grammar_sampler: Option<LlamaSampler>,
) -> Result<String> {
    let formatted_prompt = format!("<|turn>user\n{}<turn|>\n<|turn>model\n", prompt);
    let mut tokens = agent_ctx.model.str_to_token(&formatted_prompt, AddBos::Never)?;

    if tokens.is_empty() {
        tokens = agent_ctx.model.str_to_token("hi", AddBos::Never)?;
    }

    // Truncate to avoid context exhaustion (use dynamic n_ctx, leave max_new_tokens for generation)
    // Use i32 logic to prevent unsigned underflow panics/wraps.
    let base_n_past = agent_ctx.base_n_past as i32;
    let n_ctx = agent_ctx.n_ctx as i32;
    let max_gen = agent_ctx.max_new_tokens as i32;
    let max_tokens = (n_ctx - base_n_past - max_gen).max(16) as usize;
    if tokens.len() > max_tokens {
        tokens.truncate(max_tokens);
    }

    if tokens.is_empty() {
        return Err(anyhow::anyhow!("Tokens list is empty after truncation"));
    }

    let mut batch = LlamaBatch::new(n_ctx as usize, 1);
    let mut current_pos = agent_ctx.base_n_past as i32;
    let last_index = tokens.len() - 1;
    for (i, token) in tokens.into_iter().enumerate() {
        let is_last = i == last_index;
        batch.add(token, current_pos, &[0], is_last)?;
        current_pos += 1;
    }

    let mut guard = KVCacheGuard {
        ctx: &mut agent_ctx.ctx,
        base_n_past: agent_ctx.base_n_past,
    };

    guard.ctx.decode(&mut batch)?;

    let mut result_string = String::new();
    let mut n_cur = current_pos;

    let mut samplers = Vec::new();
    samplers.push(LlamaSampler::penalties(64, repetition_penalty, 0.0, 0.0));
    if let Some(g_sampler) = grammar_sampler {
        samplers.push(g_sampler);
    }
    if temperature <= 0.0 {
        samplers.push(LlamaSampler::greedy());
    } else {
        samplers.push(LlamaSampler::temp(temperature));
        samplers.push(LlamaSampler::dist(42));
    }
    let mut sampler = LlamaSampler::chain_simple(samplers);

    let turn_end_tokens = agent_ctx.model.str_to_token("<turn|>", AddBos::Never).unwrap_or_default();
    let turn_end_token = turn_end_tokens.first().copied();

    let n_len = agent_ctx.max_new_tokens; // max length
    let mut bytes_accumulator = Vec::new();

    for _ in 0..n_len {
        let new_token_id = sampler.sample(&mut guard.ctx, batch.n_tokens() - 1);

        if new_token_id == agent_ctx.model.token_eos() || Some(new_token_id) == turn_end_token {
            break;
        }

        let mut token_bytes = agent_ctx.model.token_to_piece_bytes(new_token_id, 16, false, None).unwrap_or(vec![]);
        bytes_accumulator.append(&mut token_bytes);

        process_token_bytes(&mut bytes_accumulator, &mut result_string, window);

        batch.clear();
        batch.add(new_token_id, n_cur, &[0], true)?;
        n_cur += 1;

        guard.ctx.decode(&mut batch)?;
    }

    if !bytes_accumulator.is_empty() {
        result_string.push_str(&String::from_utf8_lossy(&bytes_accumulator));
    }

    Ok(result_string)
}

