use crate::llm::llm_manager::AgentContext;
use crate::llm::worker::Route;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::llama_backend::LlamaBackend;
use std::sync::Arc;
use llama_cpp_2::sampling::LlamaSampler;
use serde::Deserialize;
use std::str::FromStr;

const ROUTER_PROMPT: &str = include_str!("../prompts/router.txt");

const MAX_NEW_TOKENS: u32 = 256;
const N_CTX: u32 = 4096;

#[derive(Debug, Clone)]
pub struct RouteResult {
    pub routes: Vec<Route>,
    pub subsequent_task: Option<String>,
    pub confidence: f32,
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
                "first_route": { "type": "string", "enum": ["INVESTIGATE", "KNOWLEDGE", "ANALYSIS", "PLOTTER", "BUILDER"] },
                "subsequent_route": { "type": "string", "enum": ["INVESTIGATE", "KNOWLEDGE", "ANALYSIS", "PLOTTER", "BUILDER", "NONE"] },
                "subsequent_task": { "type": "string" },
                "confidence": { "type": "number", "minimum": 0.0, "maximum": 1.0 }
            },
            "required": ["first_route", "subsequent_route", "subsequent_task", "confidence"]
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

#[derive(Deserialize, Debug)]
struct RouterJsonResponse {
    first_route: String,
    subsequent_route: String,
    subsequent_task: String,
    confidence: f32,
}

fn clean_json_str(output: &str) -> &str {
    let trimmed = output.trim();
    if trimmed.starts_with("```json") && trimmed.ends_with("```") {
        trimmed["```json".len()..trimmed.len() - 3].trim()
    } else if trimmed.starts_with("```") && trimmed.ends_with("```") {
        trimmed[3..trimmed.len() - 3].trim()
    } else {
        trimmed
    }
}

fn to_route_result(parsed: RouterJsonResponse) -> RouteResult {
    let first = Route::from_str(&parsed.first_route).unwrap();
    let subsequent = Route::from_str(&parsed.subsequent_route).unwrap();
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
        confidence: parsed.confidence,
    }
}

pub fn parse_route_output(output: &str) -> RouteResult {
    let clean_json = clean_json_str(output);

    if let Ok(parsed) = serde_json::from_str::<RouterJsonResponse>(clean_json) {
        to_route_result(parsed)
    } else {
        fallback_parse_route_output(output)
    }
}

fn fallback_parse_route_output(output: &str) -> RouteResult {
    let clean_json = clean_json_str(output);

    if let Ok(repaired_str) = jsonrepair_rs::jsonrepair(clean_json) {
        if let Ok(parsed) = serde_json::from_str::<RouterJsonResponse>(&repaired_str) {
            return to_route_result(parsed);
        }
    }

    RouteResult {
        routes: vec![Route::None],
        subsequent_task: None,
        confidence: 0.0,
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
            "subsequent_task": "Check network connectivity",
            "confidence": 0.9
        }"#;
        let res = parse_route_output(json_input);
        assert_eq!(res.routes, vec![Route::Knowledge, Route::Analysis]);
        assert_eq!(res.subsequent_task, Some("Check network connectivity".to_string()));
        assert!((res.confidence - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_route_output_json_markdown() {
        let markdown_input = r#"```json
        {
            "first_route": "INVESTIGATE",
            "subsequent_route": "NONE",
            "subsequent_task": "NONE",
            "confidence": 0.8
        }
        ```"#;
        let res = parse_route_output(markdown_input);
        assert_eq!(res.routes, vec![Route::Investigate]);
        assert_eq!(res.subsequent_task, None);
        assert!((res.confidence - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_route_output_fallback_repair() {
        // Missing closing brace, trailing comma, single quotes
        let fallback_input = r#"{
            'first_route': 'ANALYSIS',
            'subsequent_route': 'INVESTIGATE',
            'subsequent_task': 'Troubleshoot OSPF',
            'confidence': 0.7,
        "#;
        let res = parse_route_output(fallback_input);
        assert_eq!(res.routes, vec![Route::Analysis, Route::Investigate]);
        assert_eq!(res.subsequent_task, Some("Troubleshoot OSPF".to_string()));
        assert!((res.confidence - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_route_output_fallback_failure() {
        let invalid_input = "This is completely garbage text that cannot be repaired into a JSON object.";
        let res = parse_route_output(invalid_input);
        assert_eq!(res.routes, vec![Route::None]);
        assert_eq!(res.subsequent_task, None);
        assert!((res.confidence - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_route_from_str() {
        assert_eq!(Route::from_str("knowledge").unwrap(), Route::Knowledge);
        assert_eq!(Route::from_str("ANALYSIS").unwrap(), Route::Analysis);
        assert_eq!(Route::from_str("none").unwrap(), Route::None);
        assert_eq!(Route::from_str("ploter").unwrap(), Route::Plotter);
        assert_eq!(Route::from_str("PLOTTER").unwrap(), Route::Plotter);
        assert_eq!(Route::from_str("anything_else").unwrap(), Route::Investigate);
    }
}

