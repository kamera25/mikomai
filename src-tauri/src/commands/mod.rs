//! Thin inbound command adapters.
//!
//! Existing commands remain available during migration; new commands should
//! be added here and delegate to a core service instead of owning policy.
pub mod chat;
pub mod choices;
pub mod connections;
pub mod history;
pub mod model;
pub mod network;
pub mod operations;
pub mod rag;
pub mod settings;
pub mod tasks;
pub mod watch;
