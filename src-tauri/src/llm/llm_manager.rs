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

pub const ROUTER_PROMPT: &str = "You are a professional router for a network operator assistant MIKOMAI.
Analyze the user request and classify it into one of the following worker categories.
Your options are:
- INVESTIGATE: For requests requiring tools (ping, traceroute, show command, host list, ARP table, querying network database via query_nw_db, etc.), retrieving device stats, or gathering facts from the network.
- KNOWLEDGE: For requests requiring general explanations of network concepts, explaining OSPF/BGP/VLAN protocols, theoretical questions, or explaining general network administration terms where no real-time commands or DB queries are needed.
- ANALYSIS: For troubleshooting requests, analyzing logs, debugging errors (e.g. \"OSPF down\"), or breaking down a specific technical problem logically.

You must respond with ONLY the single word in uppercase: 'INVESTIGATE', 'KNOWLEDGE', or 'ANALYSIS'. Do not include any markdown backticks, explanations, punctuation, or other text.";

pub const INVESTIGATE_WORKER_PROMPT: &str = "You are an investigator. Gather facts from the network and database. If needed, call the appropriate tools (network_ping, network_traceroute, network_show, network_get_hosts, query_nw_db, network_arp, network_get_ip_info) to retrieve real-time details.";
pub const KNOWLEDGE_WORKER_PROMPT: &str = "You are a knowledge expert. Provide domain-specific insights, explanations, and background theory for network concepts.";
pub const ANALYSIS_WORKER_PROMPT: &str = "You are an analyst. Break down the problem logically, analyze logs/outputs, and troubleshoot issues to identify root causes.";

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

        let formatted_sys = format!("<|turn>system\n{}<turn|>\n", system_prompt);
        let mut tokens = shared.model.str_to_token(&formatted_sys, AddBos::Always)?;

        // Ensure system prompt doesn't exceed 1200 tokens to leave room for user query + generation
        let max_sys_tokens = 1200;
        if tokens.len() > max_sys_tokens {
            tokens.truncate(max_sys_tokens);
        }

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

fn process_token_bytes(
    bytes_accumulator: &mut Vec<u8>,
    result_string: &mut String,
    window: Option<&tauri::Window>,
) {
    match std::str::from_utf8(bytes_accumulator) {
        Ok(s) => {
            if let Some(w) = window {
                let _ = w.emit("llm-chunk", s);
            }
            result_string.push_str(s);
            bytes_accumulator.clear();
        }
        Err(e) => {
            let utf8_error_index = e.valid_up_to();
            let valid_str = String::from_utf8_lossy(&bytes_accumulator[..utf8_error_index]).to_string();
            if let Some(w) = window {
                let _ = w.emit("llm-chunk", &valid_str);
            }
            result_string.push_str(&valid_str);
            bytes_accumulator.drain(..utf8_error_index);
            if bytes_accumulator.len() > 8 {
                 result_string.push_str(&String::from_utf8_lossy(bytes_accumulator));
                 bytes_accumulator.clear();
            }
        }
    }
}

pub fn run_inference<'a>(
    agent_ctx: &mut AgentContext<'a>,
    model: &LlamaModel,
    prompt: &str,
    window: Option<&tauri::Window>,
    temperature: f32,
    repetition_penalty: f32,
) -> Result<String> {
    let formatted_prompt = format!("<|turn>user\n{}<turn|>\n<|turn>model\n", prompt);
    let mut tokens = model.str_to_token(&formatted_prompt, AddBos::Never)?;

    if tokens.is_empty() {
        tokens = model.str_to_token("hi", AddBos::Never)?;
    }

    // Truncate to avoid context exhaustion (n_ctx is 2048, leave 512 for generation)
    // Use i32 logic to prevent unsigned underflow panics/wraps.
    let base_n_past = agent_ctx.base_n_past as i32;
    let max_tokens = (2048 - base_n_past - 512).max(16) as usize;
    if tokens.len() > max_tokens {
        tokens.truncate(max_tokens);
    }

    if tokens.is_empty() {
        return Err(anyhow::anyhow!("Tokens list is empty after truncation"));
    }

    let mut batch = LlamaBatch::new(2048, 1);
    let mut current_pos = agent_ctx.base_n_past as i32;
    let last_index = tokens.len() - 1;
    for (i, token) in tokens.into_iter().enumerate() {
        let is_last = i == last_index;
        batch.add(token, current_pos, &[0], is_last)?;
        current_pos += 1;
    }

    agent_ctx.ctx.decode(&mut batch)?;

    let mut result_string = String::new();
    let mut n_cur = batch.n_tokens();

    let sampler = if temperature <= 0.0 {
        LlamaSampler::chain_simple([
            LlamaSampler::penalties(64, repetition_penalty, 0.0, 0.0),
            LlamaSampler::greedy(),
        ])
    } else {
        LlamaSampler::chain_simple([
            LlamaSampler::penalties(64, repetition_penalty, 0.0, 0.0),
            LlamaSampler::temp(temperature),
            LlamaSampler::dist(42),
        ])
    };
    let mut sampler = sampler;

    let turn_end_tokens = model.str_to_token("<turn|>", AddBos::Never).unwrap_or_default();
    let turn_end_token = turn_end_tokens.first().copied();

    let n_len = 500; // max length
    let mut bytes_accumulator = Vec::new();

    for _ in 0..n_len {
        let new_token_id = sampler.sample(&mut agent_ctx.ctx, batch.n_tokens() - 1);

        if new_token_id == model.token_eos() || Some(new_token_id) == turn_end_token {
            break;
        }

        let mut token_bytes = model.token_to_piece_bytes(new_token_id, 16, false, None).unwrap_or(vec![]);
        bytes_accumulator.append(&mut token_bytes);

        process_token_bytes(&mut bytes_accumulator, &mut result_string, window);

        batch.clear();
        batch.add(new_token_id, n_cur, &[0], true)?;
        n_cur += 1;

        agent_ctx.ctx.decode(&mut batch)?;
    }

    if !bytes_accumulator.is_empty() {
        result_string.push_str(&String::from_utf8_lossy(&bytes_accumulator));
    }

    // 【最重要】推論終了後、伸びてしまったKVキャッシュを巻き戻す (Truncate)
    agent_ctx.ctx.clear_kv_cache_seq(Some(0), Some(agent_ctx.base_n_past), None)?;

    Ok(result_string)
}

