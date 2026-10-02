//! Infrastructure adapters for persistence, knowledge, inference, and devices.
pub mod audit;
pub mod device;
pub mod e5_embedder;
pub mod headless;
pub mod inference;
pub mod knowledge;
pub mod memory;
pub mod persistence;
pub mod portable_device;
pub mod portable_graph;
pub mod portable_rag;
pub mod portable_watch;
pub mod python;
pub mod reporter;
pub mod search;
pub mod storage;
pub mod transfer;

pub mod attachments;
pub mod local_llama;
