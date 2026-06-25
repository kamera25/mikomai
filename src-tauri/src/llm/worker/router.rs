use crate::llm::llm_manager::AgentContext;
use crate::llm::worker::Route;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::llama_backend::LlamaBackend;
use std::sync::Arc;
use llama_cpp_2::sampling::LlamaSampler;
use serde::Deserialize;

const ROUTER_PROMPT: &str = include_str!("../prompts/router.txt");

const MAX_NEW_TOKENS: u32 = 256;
const N_CTX: u32 = 2048;

#[derive(Debug, Clone)]
pub struct RouteResult {
    pub routes: Vec<Route>,
    pub subsequent_task: Option<String>,
}

pub struct Router {
    pub ctx: AgentContext,
}

impl Router {
    pub fn new(model: &Arc<LlamaModel>, backend: &Arc<LlamaBackend>) -> Result<Self, String> {
        let ctx = AgentContext::new(model.clone(), backend.clone(), ROUTER_PROMPT, 0, MAX_NEW_TOKENS, N_CTX)
            .map_err(|e| format!("Failed to create router context: {:?}", e))?;
        
        Ok(Self { ctx })
    }

    pub fn route(
        &mut self,
        _model: &Arc<LlamaModel>,
        query: &str,
        repetition_penalty: f32,
    ) -> Result<RouteResult, String> {
        let schema = r#"{
            "type": "object",
            "properties": {
                "first_route": { "type": "string", "enum": ["INVESTIGATE", "KNOWLEDGE", "ANALYSIS", "PLOTER"] },
                "subsequent_route": { "type": "string", "enum": ["INVESTIGATE", "KNOWLEDGE", "ANALYSIS", "PLOTER", "NONE"] },
                "subsequent_task": { "type": "string" }
            },
            "required": ["first_route", "subsequent_route", "subsequent_task"]
        }"#;

        let grammar_str = llama_cpp_2::json_schema_to_grammar(schema)
            .map_err(|e| format!("Failed to convert schema to grammar: {:?}", e))?;

        let grammar_sampler = LlamaSampler::grammar(&self.ctx.model, &grammar_str, "root")
            .map_err(|e| format!("Failed to create grammar sampler: {:?}", e))?;

        let route_output = crate::llm::llm_manager::run_inference_with_grammar(
            &mut self.ctx,
            query,
            None,
            0.0,
            repetition_penalty,
            Some(grammar_sampler),
        ).map_err(|e| format!("Routing inference failed: {:?}", e))?;

        Ok(parse_route_output(&route_output))
    }
}

pub fn parse_route_output(output: &str) -> RouteResult {
    #[derive(Deserialize, Debug)]
    struct RouterJsonResponse {
        first_route: String,
        subsequent_route: String,
        subsequent_task: String,
    }

    let trimmed = output.trim();
    let clean_json = if trimmed.starts_with("```json") && trimmed.ends_with("```") {
        trimmed["```json".len()..trimmed.len() - 3].trim()
    } else if trimmed.starts_with("```") && trimmed.ends_with("```") {
        trimmed[3..trimmed.len() - 3].trim()
    } else {
        trimmed
    };

    if let Ok(parsed) = serde_json::from_str::<RouterJsonResponse>(clean_json) {
        let first = Route::from_str(&parsed.first_route);
        let subsequent = Route::from_str(&parsed.subsequent_route);
        let subsequent_task = {
            let task_val = parsed.subsequent_task.trim();
            if task_val.is_empty() || task_val.to_uppercase() == "NONE" {
                None
            } else {
                Some(task_val.to_string())
            }
        };

        let mut routes = vec![first];
        if subsequent != Route::None {
            routes.push(subsequent);
        }

        RouteResult {
            routes,
            subsequent_task,
        }
    } else {
        fallback_parse_route_output(output)
    }
}

fn fallback_parse_route_output(output: &str) -> RouteResult {
    let mut first_route = Route::Investigate;
    let mut subsequent_route = Route::None;
    let mut subsequent_task = None;

    for line in output.lines() {
        let trimmed = line.trim();
        let trimmed_upper = trimmed.to_uppercase();
        if trimmed_upper.starts_with("FIRST_ROUTE:") {
            let val = trimmed["FIRST_ROUTE:".len()..].trim();
            first_route = Route::from_str(val);
        } else if trimmed_upper.starts_with("SUBSEQUENT_ROUTE:") {
            let val = trimmed["SUBSEQUENT_ROUTE:".len()..].trim();
            subsequent_route = Route::from_str(val);
        } else if trimmed_upper.starts_with("TASK:") {
            let val = trimmed["TASK:".len()..].trim();
            let val_upper = val.to_uppercase();
            if val_upper != "NONE" && !val.is_empty() {
                subsequent_task = Some(val.to_string());
            }
        }
    }

    if !output.to_uppercase().contains("FIRST_ROUTE:") {
        first_route = Route::from_str(output);
    }

    if subsequent_route == Route::None && subsequent_task.is_some() {
        subsequent_route = Route::Analysis;
    }

    let mut routes = vec![first_route];
    if subsequent_route != Route::None {
        routes.push(subsequent_route);
    }

    RouteResult {
        routes,
        subsequent_task,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_route_output_json() {
        let json_input = r#"{
            "first_route": "KNOWLEDGE",
            "subsequent_route": "ANALYSIS",
            "subsequent_task": "Check network connectivity"
        }"#;
        let res = parse_route_output(json_input);
        assert_eq!(res.routes, vec![Route::Knowledge, Route::Analysis]);
        assert_eq!(res.subsequent_task, Some("Check network connectivity".to_string()));
    }

    #[test]
    fn test_parse_route_output_json_markdown() {
        let markdown_input = r#"```json
        {
            "first_route": "INVESTIGATE",
            "subsequent_route": "NONE",
            "subsequent_task": "NONE"
        }
        ```"#;
        let res = parse_route_output(markdown_input);
        assert_eq!(res.routes, vec![Route::Investigate]);
        assert_eq!(res.subsequent_task, None);
    }

    #[test]
    fn test_parse_route_output_fallback() {
        let fallback_input = "FIRST_ROUTE: ANALYSIS\nSUBSEQUENT_ROUTE: INVESTIGATE\nTASK: Troubleshoot OSPF";
        let res = parse_route_output(fallback_input);
        assert_eq!(res.routes, vec![Route::Analysis, Route::Investigate]);
        assert_eq!(res.subsequent_task, Some("Troubleshoot OSPF".to_string()));
    }

    #[test]
    fn test_route_from_str() {
        assert_eq!(Route::from_str("knowledge"), Route::Knowledge);
        assert_eq!(Route::from_str("ANALYSIS"), Route::Analysis);
        assert_eq!(Route::from_str("none"), Route::None);
        assert_eq!(Route::from_str("ploter"), Route::Ploter);
        assert_eq!(Route::from_str("PLOTTER"), Route::Ploter);
        assert_eq!(Route::from_str("anything_else"), Route::Investigate);
    }
}

