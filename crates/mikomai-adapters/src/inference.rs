//! LLM and embedding adapters. Concrete llama/fastembed implementations stay
//! behind this boundary so the core never owns model lifetime.
pub trait Inference: Send + Sync {
    fn complete(&self, prompt: &str) -> Result<String, String>;
}

/// Runtime-neutral Llama adapter. The desktop crate supplies the model
/// callback; model lifetime and cancellation stay outside the core contract.
pub struct LlamaAdapter<F> {
    pub model_path: Option<std::path::PathBuf>,
    pub complete_fn: F,
}
impl<F> LlamaAdapter<F> {
    pub fn new(model_path: Option<std::path::PathBuf>, complete_fn: F) -> Self {
        Self {
            model_path,
            complete_fn,
        }
    }
}
impl<F> mikomai_core::port::InferencePort for LlamaAdapter<F>
where
    F: Fn(&str) -> Result<String, String> + Send + Sync,
{
    fn complete<'a>(&'a self, prompt: &'a str) -> mikomai_core::port::PortFuture<'a, String> {
        Box::pin(async move { (self.complete_fn)(prompt) })
    }
}

pub struct FnInference<F>(pub F);
impl<F> mikomai_core::port::InferencePort for FnInference<F>
where
    F: Fn(&str) -> Result<String, String> + Send + Sync,
{
    fn complete<'a>(&'a self, prompt: &'a str) -> mikomai_core::port::PortFuture<'a, String> {
        Box::pin(async move { (self.0)(prompt) })
    }
}

impl<F> Inference for FnInference<F>
where
    F: Fn(&str) -> Result<String, String> + Send + Sync,
{
    fn complete(&self, prompt: &str) -> Result<String, String> {
        (self.0)(prompt)
    }
}
