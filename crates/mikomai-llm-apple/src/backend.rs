use crate::ffi::Session;
use mikomai_llm::{
    InferenceCapabilities, InferencePort, ModelAvailability, PortFuture, TokenLimits,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};

pub const CONTEXT_WINDOW: u32 = 4096;
static CANCELLED: AtomicBool = AtomicBool::new(false);

/// Stop queued generation and discard an in-flight response when it finishes.
/// The minimal native ABI does not interrupt Foundation Models mid-generation.
pub fn cancel() {
    CANCELLED.store(true, Ordering::Relaxed);
}
pub fn reset_cancellation() {
    CANCELLED.store(false, Ordering::Relaxed);
}

type Reply = mpsc::Sender<Result<String, String>>;
struct Request {
    prompt: String,
    schema: Option<String>,
    reply: Reply,
}
struct Worker {
    sender: Option<mpsc::Sender<Request>>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    fn new(instructions: String) -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel::<Request>();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("mikomai-foundation-models".into())
            .spawn(move || {
                // The opaque handle never leaves this thread, including on destruction.
                let mut session = match Session::new(&instructions) {
                    Ok(session) => session,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };
                if ready_tx.send(Ok(())).is_err() {
                    return;
                }
                for request in receiver {
                    let result = if CANCELLED.load(Ordering::Relaxed) {
                        Err("生成を停止しました。".into())
                    } else {
                        let result = session.respond_structured(&request.prompt, request.schema.as_deref());
                        if CANCELLED.load(Ordering::Relaxed) {
                            Err("生成を停止しました。".into())
                        } else {
                            result
                        }
                    };
                    let _ = request.reply.send(result);
                }
                // Session::drop releases the retained Swift object here.
            })
            .map_err(|e| format!("Cannot start Apple Foundation Models worker: {e}"))?;
        let worker = Self {
            sender: Some(sender),
            thread: Some(thread),
        };
        ready_rx
            .recv()
            .map_err(|_| "Apple Foundation Models worker stopped during creation".to_owned())??;
        Ok(worker)
    }

    fn respond(&self, prompt: &str, schema: Option<&str>) -> Result<String, String> {
        let (reply, receiver) = mpsc::channel();
        self.sender
            .as_ref()
            .expect("live worker")
            .send(Request {
                prompt: prompt.into(),
                schema: schema.map(str::to_owned),
                reply,
            })
            .map_err(|_| "Apple Foundation Models worker stopped".to_owned())?;
        let response = receiver
            .recv()
            .map_err(|_| "Apple Foundation Models worker stopped during generation".to_owned())??;
        if response.trim().is_empty() {
            return Err("Apple Foundation Models returned an empty response".into());
        }
        Ok(response)
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.sender.take(); // Close the queue before joining; no handle crosses threads.
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// One persistent native conversation, serialized by a dedicated worker.
/// Send/Sync are derived from Rust channels, never asserted for a native handle.
/// Generation and Drop wait synchronously. Poll complete() on a blocking thread
/// when integrating with an async executor or a UI event loop.
/// Availability reports initialization status; subsequent native failures are
/// returned by respond/complete. A new instance starts a new conversation.
pub struct AppleFoundationModel {
    worker: Result<Worker, String>,
}
/// Compatibility name used by existing Mikomai callers.
pub type AppleInference = AppleFoundationModel;

impl AppleFoundationModel {
    /// Initialize a native session with system instructions, or return its error.
    pub fn new(instructions: &str) -> Result<Self, String> {
        Ok(Self {
            worker: Ok(Worker::new(instructions.to_owned())?),
        })
    }

    pub fn respond(&self, prompt: &str) -> Result<String, String> {
        self.worker.as_ref().map_err(Clone::clone)?.respond(prompt, None)
    }

    /// Generate typed JSON through Foundation Models guided generation.
    /// A fresh backend/session should be used for each independent selection.
    pub fn respond_structured(&self, prompt: &str, schema: &str) -> Result<String, String> {
        self.worker.as_ref().map_err(Clone::clone)?.respond(prompt, Some(schema))
    }

    /// Keep existing callers compiling; Foundation Models enforces the actual
    /// cumulative context limit. No guessed tokenizer or silent truncation.
    pub fn complete_with_context(
        &self,
        required: &str,
        context: &[&str],
    ) -> Result<String, String> {
        let prompt = context
            .iter()
            .copied()
            .chain(std::iter::once(required))
            .collect::<Vec<_>>()
            .join("\n\n");
        self.respond(&prompt)
    }
}
impl Default for AppleFoundationModel {
    fn default() -> Self {
        Self {
            worker: Worker::new(String::new()),
        }
    }
}
impl InferencePort for AppleFoundationModel {
    fn complete<'a>(&'a self, prompt: &'a str) -> PortFuture<'a, String> {
        Box::pin(async move { self.respond(prompt) })
    }
    fn capabilities(&self) -> InferenceCapabilities {
        InferenceCapabilities {
            structured_output: true,
            token_limits: TokenLimits {
                context_window: Some(CONTEXT_WINDOW),
                max_output_tokens: None,
            },
            ..InferenceCapabilities::default()
        }
    }
    fn availability(&self) -> ModelAvailability {
        match &self.worker {
            Ok(_) => ModelAvailability::Available,
            Err(reason) => ModelAvailability::Unavailable {
                reason: reason.clone(),
            },
        }
    }
    fn cancel(&self) {
        cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn implements_existing_send_sync_port() {
        fn assert_port<T: InferencePort>() {}
        assert_port::<AppleFoundationModel>();
    }
    #[test]
    fn initialization_errors_reach_the_caller() {
        assert!(AppleFoundationModel::new("bad\0instructions")
            .err()
            .unwrap()
            .contains("NUL"));
    }
    #[test]
    #[ignore = "requires an available Apple Intelligence system model"]
    fn native_session_remembers_previous_turn_and_instructions() {
        let backend = AppleFoundationModel::new(
            "The system code is 482619. Remember it and any user code provided in the conversation.",
        )
        .unwrap();
        let first = backend
            .respond("My user code is 731946. Remember it.")
            .unwrap();
        assert!(!first.trim().is_empty());
        let second = futures_lite::future::block_on(
            backend.complete("What are the user code and the system code?"),
        )
        .unwrap();
        // Verify information from both instructions and a previous turn, without
        // requiring nondeterministic model output to use a particular phrasing.
        assert!(second.contains("731946"), "conversation was lost: {second}");
        assert!(
            second.contains("482619"),
            "instructions were lost: {second}"
        );
        assert!(backend.respond("bad\0prompt").unwrap_err().contains("NUL"));
        // Drop waits for worker shutdown and destroys the session on its owner thread.
    }
}
