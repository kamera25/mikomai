//! Common LLM contract. Reuse the domain's existing ports rather than add
//! another generation interface. No backend implementation is selected here.
pub use mikomai_core::port::{
    InferenceCapabilities, InferencePort, ModelAvailability, PortFuture, StreamingInferencePort,
    TokenLimits, VisionPort,
};
