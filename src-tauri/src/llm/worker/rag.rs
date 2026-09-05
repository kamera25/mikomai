use crate::llm::llm::SYSTEM_PROMPT;
use crate::llm::llm_manager::AgentContext;
use crate::llm::worker::LlmWorker;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::sampling::LlamaSampler;
use serde::Deserialize;
use std::sync::Arc;

const RAG_WORKER_PROMPT: &str = include_str!("../prompts/rag_worker.txt");

const MAX_NEW_TOKENS: u32 = 512;
const N_CTX: u32 = 8192;
const SELECTOR_MAX_NEW_TOKENS: u32 = 192;
const DOCUMENT_SELECTION_SCHEMA: &str = r#"{
  "type": "object",
  "properties": { "paths": { "type": "array", "items": { "type": "string" }, "maxItems": 3 } },
  "required": ["paths"]
}"#;

#[derive(Debug, Deserialize)]
struct DocumentSelection {
    paths: Vec<String>,
}

/// Select evidence using only document metadata. Bodies are fetched only after
/// this constrained decision, keeping the final answer grounded and focused.
pub fn select_documents(
    model: &Arc<LlamaModel>,
    backend: &Arc<LlamaBackend>,
    user_message: &str,
    previews: &[crate::mcp::rag::RagDocumentPreview],
    temperature: f32,
    repetition_penalty: f32,
) -> Result<Vec<String>, String> {
    if previews.is_empty() { return Ok(Vec::new()); }
    let catalog = serde_json::to_string(previews)
        .map_err(|error| format!("Failed to serialize RAG document catalog: {error}"))?;
    let prompt = format!(
        "ユーザーの質問: {user_message}\n\n候補資料（本文は未読）: {catalog}\n\n質問に直接答えるために必要な資料の path を最大3件だけ選んでください。候補外のpathは絶対に出力せず、不要なら空配列にしてください。"
    );
    let mut context = AgentContext::new(
        model.clone(), backend.clone(),
        "You select relevant network-document sources. Return only valid JSON.",
        6, SELECTOR_MAX_NEW_TOKENS, 4096,
    ).map_err(|error| format!("Failed to create RAG document selector: {error:?}"))?;
    let grammar = llama_cpp_2::json_schema_to_grammar(DOCUMENT_SELECTION_SCHEMA)
        .map_err(|error| format!("Failed to create RAG selector grammar: {error:?}"))?;
    let sampler = LlamaSampler::grammar(&context.model, &grammar, "root")
        .map_err(|error| format!("Failed to create RAG selector sampler: {error:?}"))?;
    let output = crate::llm::llm_manager::run_inference_with_grammar(
        &mut context, &prompt, None, temperature, repetition_penalty, Some(sampler),
    ).map_err(|error| format!("RAG document selection failed: {error:?}"))?;
    let selection: DocumentSelection = serde_json::from_str(output.trim())
        .map_err(|error| format!("RAG document selector returned invalid JSON: {error}"))?;
    let allowed: std::collections::HashSet<_> = previews.iter().map(|preview| preview.path.as_str()).collect();
    Ok(selection.paths.into_iter().filter(|path| allowed.contains(path.as_str())).take(3).collect())
}

pub struct RagWorker {
    pub ctx: Option<AgentContext>,
}

impl RagWorker {
    pub fn new(
        model: &Arc<LlamaModel>,
        backend: &Arc<LlamaBackend>,
        preload: bool,
    ) -> Result<Self, String> {
        if preload {
            let full_system_prompt = format!(
                "{}\n\n=== Current Role ===\nあなたは現在「RAG Worker (RAG回答員)」として動作しています。以下の役割指示に特化してください:\n{}",
                SYSTEM_PROMPT,
                RAG_WORKER_PROMPT
            );
            let ctx = AgentContext::new(
                model.clone(),
                backend.clone(),
                &full_system_prompt,
                4,
                MAX_NEW_TOKENS,
                N_CTX,
            )
            .map_err(|e| format!("Failed to create Rag context: {:?}", e))?;

            Ok(Self { ctx: Some(ctx) })
        } else {
            Ok(Self { ctx: None })
        }
    }
}

impl LlmWorker for RagWorker {
    fn agent_name(&self) -> &'static str {
        "RAG Worker (RAG回答員)"
    }

    fn context_mut(&mut self) -> &mut AgentContext {
        self.ctx.as_mut().expect("Rag context not initialized")
    }

    fn ensure_initialized(
        &mut self,
        model: &Arc<LlamaModel>,
        backend: &Arc<LlamaBackend>,
    ) -> Result<(), String> {
        if self.ctx.is_none() {
            let full_system_prompt = format!(
                "{}\n\n=== Current Role ===\nあなたは現在「RAG Worker (RAG回答員)」として動作しています。以下の役割指示に特化してください:\n{}",
                SYSTEM_PROMPT,
                RAG_WORKER_PROMPT
            );
            let ctx = AgentContext::new(
                model.clone(),
                backend.clone(),
                &full_system_prompt,
                4,
                MAX_NEW_TOKENS,
                N_CTX,
            )
            .map_err(|e| format!("Failed to create Rag context: {:?}", e))?;

            self.ctx = Some(ctx);
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
                "ユーザーの質問: \"{}\"\nに対して、LLMが選択して展開した技術文書データベース(NW-DB)の本文を以下に示します:\n\n{}\n\n本文に記載されたコマンド・前提条件・手順を使って、ユーザーへ直接回答してください。『資料を参照してください』と案内するだけの回答や、同じ説明の反復は禁止です。本文にない内容は推測せず、不足を明記してください。{}",
                user_msg, out, hist
            )
        }
    }
}
