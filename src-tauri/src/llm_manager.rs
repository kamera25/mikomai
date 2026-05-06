use anyhow::Result;
use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_batch::LlamaBatch;
use std::sync::Arc;

pub const ROUTER_PROMPT: &str = "You are a router. Analyze the user request and choose the appropriate worker.";
pub const INVESTIGATE_WORKER_PROMPT: &str = "You are an investigator. Gather facts from the provided context.";
pub const KNOWLEDGE_WORKER_PROMPT: &str = "You are a knowledge expert. Provide domain-specific insights.";
pub const ANALYSIS_WORKER_PROMPT: &str = "You are an analyst. Break down the problem logically.";

pub struct SharedModel {
    pub model: Arc<LlamaModel>,
    pub backend: Arc<LlamaBackend>,
}

pub struct AgentManager<'a> {
    pub router_ctx: AgentContext<'a>,
    pub investigate_worker_ctx: AgentContext<'a>,
    pub knowledge_worker_ctx: AgentContext<'a>,
    pub analysis_worker_ctx: AgentContext<'a>,
}

pub struct AgentContext<'a> {
    pub ctx: LlamaContext<'a>,
    pub base_n_past: u32,
    pub id: i32,
}

impl<'a> AgentManager<'a> {
    pub fn new(shared: &'a SharedModel) -> Result<Self> {
        let router_ctx = AgentContext::new(shared, ROUTER_PROMPT, 0)?;
        let investigate_worker_ctx = AgentContext::new(shared, INVESTIGATE_WORKER_PROMPT, 1)?;
        let knowledge_worker_ctx = AgentContext::new(shared, KNOWLEDGE_WORKER_PROMPT, 2)?;
        let analysis_worker_ctx = AgentContext::new(shared, ANALYSIS_WORKER_PROMPT, 3)?;

        Ok(Self {
            router_ctx,
            investigate_worker_ctx,
            knowledge_worker_ctx,
            analysis_worker_ctx,
        })
    }
}

impl<'a> AgentContext<'a> {
    pub fn new(shared: &'a SharedModel, system_prompt: &str, id: i32) -> Result<Self> {
        let mut ctx_params = LlamaContextParams::default();
        ctx_params = ctx_params.with_n_ctx(std::num::NonZeroU32::new(2048));

        let mut ctx = shared.model.new_context(&shared.backend, ctx_params)?;

        let tokens = shared.model.str_to_token(system_prompt, llama_cpp_2::model::AddBos::Always)?;

        let mut batch = LlamaBatch::new(2048, 1);
        let last_index = tokens.len() - 1;

        for (i, token) in tokens.into_iter().enumerate() {
            let is_last = i == last_index;
            batch.add(token, i as i32, &[0], is_last)?;
        }

        ctx.decode(&mut batch)?;

        let base_n_past = batch.n_tokens() as u32;

        Ok(Self { ctx, base_n_past, id })
    }
}

pub fn run_inference<'a>(
    agent_ctx: &mut AgentContext<'a>,
    model: &LlamaModel,
    prompt: &str
) -> Result<String> {
    let tokens = model.str_to_token(prompt, llama_cpp_2::model::AddBos::Never)?;
    let mut batch = LlamaBatch::new(2048, 1);

    let mut current_pos = agent_ctx.base_n_past as i32;
    let last_index = tokens.len() - 1;
    for (i, token) in tokens.into_iter().enumerate() {
        let is_last = i == last_index;
        batch.add(token, current_pos, &[0], is_last)?;
        current_pos += 1;
    }

    agent_ctx.ctx.decode(&mut batch)?;

    let generated_text = String::from("【推論結果のダミー文字列】");

    // 【最重要】推論終了後、伸びてしまったKVキャッシュを巻き戻す (Truncate)
    agent_ctx.ctx.clear_kv_cache_seq(Some(0), Some(agent_ctx.base_n_past), None)?;

    Ok(generated_text)
}
