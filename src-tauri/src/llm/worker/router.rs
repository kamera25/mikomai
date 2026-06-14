use crate::llm::llm_manager::AgentContext;
use crate::llm::worker::Route;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::llama_backend::LlamaBackend;

const ROUTER_PROMPT: &str = include_str!("../prompts/router.txt");

const MAX_NEW_TOKENS: u32 = 256;
const N_CTX: u32 = 2048;

#[derive(Debug, Clone)]
pub struct RouteResult {
    pub routes: Vec<Route>,
    pub subsequent_task: Option<String>,
}

pub struct Router {
    pub ctx: AgentContext<'static>,
}

impl Router {
    pub fn new(model: &LlamaModel, backend: &LlamaBackend) -> Result<Self, String> {
        let ctx = AgentContext::new(model, backend, ROUTER_PROMPT, 0, MAX_NEW_TOKENS, N_CTX)
            .map_err(|e| format!("Failed to create router context: {:?}", e))?;
        
        // Safety: We transmute LlamaContext to 'static lifetime.
        // This is safe because Router is owned by SharedModel, which also owns the LlamaModel.
        // The LlamaModel outlives the Router and LlamaContext.
        let ctx_static = unsafe {
            std::mem::transmute::<AgentContext<'_>, AgentContext<'static>>(ctx)
        };
        
        Ok(Self { ctx: ctx_static })
    }

    pub fn route(
        &mut self,
        model: &LlamaModel,
        query: &str,
        repetition_penalty: f32,
    ) -> Result<RouteResult, String> {
        let route_output = crate::llm::llm_manager::run_inference(
            &mut self.ctx,
            model,
            query,
            None,
            0.0,
            repetition_penalty,
        ).map_err(|e| format!("Routing inference failed: {:?}", e))?;

        Ok(parse_route_output(&route_output))
    }
}

pub fn parse_route_output(output: &str) -> RouteResult {
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
