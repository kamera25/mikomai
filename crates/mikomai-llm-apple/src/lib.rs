//! Native Apple Foundation Models backend (macOS 26 or later).
//! Each backend owns one persistent conversation. Raw native handles stay private.
//!
//! Cargo builds and links a Swift dylib in its build output directory. When
//! distributing an app, bundle `libMikomaiFoundationModels.dylib` and relocate
//! its install name and the executable's reference to the app's Frameworks
//! directory. Swift runtime dependencies come from macOS itself.
#![cfg(target_os = "macos")]

mod backend;
mod ffi;

pub use backend::{
    cancel, reset_cancellation, AppleFoundationModel, AppleInference, CONTEXT_WINDOW,
};
