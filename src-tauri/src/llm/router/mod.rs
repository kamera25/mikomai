pub mod llm_router;
pub mod shortcut;
pub mod types;

pub use types::*;

use crate::llm::llm::LlmError;
use crate::llm::llm_manager::SharedModel;
use crate::llm::worker::{format_device_contexts, resolve_device_contexts, Route};
use crate::settings::AppSettings;
use tauri::AppHandle;

pub struct RoutingPipeline;

impl RoutingPipeline
{
    /// Clarification message to ask the user for intent
    pub fn build_clarification_message() -> String
    {
        "ご質問の意図を確認させてください。\n\n```json\n{\n  \"tool_name\": \"ask_user_choice\",\n  \"params\": {\n    \"title\": \"ご質問の意図の確認\",\n    \"message\": \"ご質問の意図を確認させてください。以下のどれに該当しますか？\",\n    \"options\": [\n      \"1. ネットワーク機器の調査 (INVESTIGATE)\",\n      \"2. 技術知識の解説 (KNOWLEDGE)\",\n      \"3. Config作成 (BUILDER)\"\n    ]\n  }\n}\n```".to_string()
    }

    /// Primary routing method executing Shortcut -> LLM Router pipeline
    pub fn route(
        shared_model: &SharedModel,
        original_query: &str,
        settings: &AppSettings,
        app: &AppHandle,
    ) -> Result<RoutingDecision, LlmError>
    {
        let has_image_attachment = original_query.contains("【添付画像Vision解析情報")
            || original_query.contains("[添付画像:");

        // 1. Phase 1: Fast shortcut routing (regex-based)
        if !has_image_attachment
        {
            if let Some(decision) = shortcut::detect_shortcut(original_query)
            {
                if decision.confidence >= 0.8
                {
                    return Ok(decision);
                }
            }
        }

        // 2. Phase 2: LLM Router with context enrichment
        let device_contexts = resolve_device_contexts(app, original_query);
        let enriched_query = if !device_contexts.is_empty()
        {
            let enrichment = format_device_contexts(&device_contexts);
            format!("{}{}", enrichment, original_query)
        }
        else
        {
            original_query.to_string()
        };

        log::info!(
            "--- ROUTER INPUT QUERY (Enriched) ---\n{}\n-------------------------",
            enriched_query
        );

        let mut route_result = {
            let mut router_lock = shared_model.router.lock().unwrap();
            router_lock
                .route(
                    &shared_model.model,
                    &enriched_query,
                    settings.repetition_penalty,
                )
                .map_err(|e| LlmError::Routing(format!("{:?}", e)))?
        };
        route_result.device_contexts = device_contexts.clone();

        log::info!(
            "--- ROUTER OUTPUT ---\n{:?}\n-------------------------",
            route_result
        );

        // Low confidence fallback
        if route_result.confidence < 0.5
        {
            return Ok(RoutingDecision {
                action: RouteAction::AskClarification,
                confidence: route_result.confidence as f64,
                device_contexts,
                source: RoutingSource::LlmRouter,
            });
        }

        let first_route = route_result
            .routes
            .first()
            .copied()
            .unwrap_or(Route::Investigate);
        let subsequent_route = route_result.routes.get(1).copied();

        Ok(RoutingDecision {
            action: RouteAction::WorkerRoute {
                route: first_route,
                subsequent_route,
                subsequent_task: route_result.subsequent_task,
            },
            confidence: route_result.confidence as f64,
            device_contexts,
            source: RoutingSource::LlmRouter,
        })
    }
}
