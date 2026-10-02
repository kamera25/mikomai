//! Request-scoped, callback-only diagnostics. Nothing is persisted.
use crate::MikomaiStreamCallback;
use std::cell::Cell;
use std::ffi::CString;

type Sink = (MikomaiStreamCallback, usize);
thread_local! { static SINK: Cell<Option<Sink>> = const { Cell::new(None) }; }
pub struct Scope(Option<Sink>);
impl Scope {
    pub fn enter(callback: Option<MikomaiStreamCallback>, context: *mut std::ffi::c_void) -> Self {
        Self(SINK.with(|sink| sink.replace(callback.map(|cb| (cb, context as usize)))))
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        SINK.with(|sink| sink.set(self.0));
    }
}
pub fn emit(kind: &str, payload: serde_json::Value) {
    SINK.with(|sink| {
        if let Some((callback, context)) = sink.get() {
            let record = serde_json::json!({"timestamp": chrono::Utc::now(), "kind": kind, "payload": payload});
            if let Ok(text) = CString::new(format!("__MIKOMAI_DEBUG__{}", record)) {
                unsafe { callback(text.as_ptr(), 0, context as *mut std::ffi::c_void); }
            }
        }
    });
}

pub struct StreamingInference;
impl mikomai_core::port::StreamingInferencePort for StreamingInference {
    fn complete_streaming(
        &self,
        prompt: &str,
        callback: &mut dyn FnMut(&str, bool),
    ) -> Result<String, String> {
        emit(
            "llm_request",
            serde_json::json!({"backend":"llamacpp", "prompt":prompt}),
        );
        let result = mikomai_core::port::StreamingInferencePort::complete_streaming(
            &mikomai_adapters::local_llama::LocalInference,
            prompt,
            callback,
        );
        emit("llm_response", serde_json::json!({"result":result}));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    unsafe extern "C" fn capture(
        text: *const std::ffi::c_char,
        done: i32,
        context: *mut std::ffi::c_void,
    ) {
        assert_eq!(done, 0);
        let records = &mut *(context as *mut Vec<String>);
        records.push(CStr::from_ptr(text).to_str().unwrap().to_owned());
    }

    #[test]
    fn records_are_valid_json_preserve_prompt_and_restore_nested_scope() {
        let mut outer = Vec::<String>::new();
        let mut inner = Vec::<String>::new();
        let payload =
            serde_json::json!({"prompt":"日本語\n\"quoted\"\u{0000}text", "query":"VLAN"});
        {
            let _scope = Scope::enter(Some(capture), &mut outer as *mut _ as *mut _);
            emit("llm_request", payload.clone());
            {
                let _nested = Scope::enter(Some(capture), &mut inner as *mut _ as *mut _);
                emit("agent_query", payload.clone());
            }
            emit("llm_response", serde_json::json!({"answer":"ok"}));
        }
        emit("after_request", payload.clone());
        assert_eq!(outer.len(), 2);
        assert_eq!(inner.len(), 1);
        let record: serde_json::Value =
            serde_json::from_str(outer[0].strip_prefix("__MIKOMAI_DEBUG__").unwrap()).unwrap();
        assert_eq!(record["payload"], payload);
        assert_eq!(record["kind"], "llm_request");
        assert!(record["timestamp"].is_string());
    }

    #[test]
    fn scope_cannot_leak_to_other_threads_or_after_return() {
        let mut records = Vec::<String>::new();
        {
            let _scope = Scope::enter(Some(capture), &mut records as *mut _ as *mut _);
            std::thread::spawn(|| emit("other_request", serde_json::json!({})))
                .join()
                .unwrap();
            emit("this_request", serde_json::json!({}));
        }
        emit("after_return", serde_json::json!({}));
        assert_eq!(records.len(), 1);
    }
}
