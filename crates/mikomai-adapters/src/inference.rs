//! LLM and embedding adapters. Concrete llama/fastembed implementations stay
//! behind this boundary so the core never owns model lifetime.
pub trait Inference: Send + Sync { fn complete(&self, prompt: &str) -> Result<String, String>; }
