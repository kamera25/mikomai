use mikomai_core::ReportEvent;
use serde_json::{json, Value};
use std::io::Write;

pub struct Trace<W> {
    writer: W,
    enabled: bool,
    error: Option<std::io::Error>,
    task_id: String,
    seq: u64,
    journal: bool,
}

impl<W: Write> Trace<W> {
    pub fn new(writer: W, enabled: bool) -> Self {
        Self {
            writer,
            enabled,
            error: None,
            task_id: uuid::Uuid::new_v4().to_string(),
            seq: 0,
            journal: false,
        }
    }

    pub fn journaled(writer: W, enabled: bool) -> Self {
        let mut trace = Self::new(writer, enabled);
        trace.journal = enabled;
        trace
    }

    fn write_record(&mut self, record: &str) {
        if !self.enabled || self.error.is_some() {
            return;
        }
        // Flush each record so redirected output is usable during inference.
        if let Err(error) = writeln!(self.writer, "{record}").and_then(|_| self.writer.flush()) {
            self.error = Some(error);
        }
    }

    pub fn emit(&mut self, kind: &str, payload: Value) {
        if self.enabled {
            if self.journal {
                match mikomai_app::api::engine().trace_event(&self.task_id, kind, payload.to_string()) {
                    Ok(event) => self.seq = event.seq,
                    Err(error) => { self.error = Some(std::io::Error::other(error)); return; }
                }
            } else { self.seq += 1; }
            self.write_record(
                &json!({"task_id":self.task_id,"seq":self.seq,"version":1,"timestamp": chrono::Utc::now(), "kind": kind, "payload": payload})
                    .to_string(),
            );
        }
    }

    pub fn stream(&mut self, text: &str, done: bool) {
        if let Some(record) = text.strip_prefix("__MIKOMAI_DEBUG__") {
            if let Ok(value) = serde_json::from_str::<Value>(record) {
                self.emit(
                    value["kind"].as_str().unwrap_or("debug"),
                    value["payload"].clone(),
                );
            } else {
                self.emit(
                    "invalid_debug_record",
                    json!({"error":"invalid internal JSON"}),
                );
            }
        } else {
            self.emit("core_stream", json!({"text": text, "done": done}));
        }
    }

    pub fn report(&mut self, event: &ReportEvent) {
        let payload = match event {
            ReportEvent::TaskStarted { task_id } => {
                json!({"event_type": "task_started", "task_id": task_id})
            }
            ReportEvent::Status { task_id, status } => {
                json!({"event_type": "state_updated", "task_id": task_id, "status": status})
            }
            ReportEvent::Evidence { task_id, evidence } => {
                json!({"event_type": "observation", "task_id": task_id, "evidence": evidence})
            }
            ReportEvent::ApprovalRequired {
                task_id,
                plan,
                message,
            } => {
                json!({"event_type": "approval_required", "task_id": task_id, "plan": plan, "message": message})
            }
            ReportEvent::Completed { task_id, answer } => {
                json!({"event_type": "finished", "task_id": task_id, "answer": answer})
            }
        };
        self.emit("agent_event", payload);
    }

    pub fn check(&self) -> Result<(), String> {
        self.error.as_ref().map_or(Ok(()), |error| Err(format!("TaskEvent journal failed: {error}")))
    }

    pub fn finish(self) -> Result<(), String> {
        self.error.map_or(Ok(()), |error| {
            Err(format!("JSONL stdout write failed: {error}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_preserves_core_records_and_escapes_multiline_chunks() {
        let mut output = Vec::new();
        let mut trace = Trace::new(&mut output, true);
        let core = json!({"timestamp": "2026-10-02T00:00:00Z", "kind": "llm_request", "payload": {"prompt": "日本語\n\"quoted\""}}).to_string();
        trace.stream(&format!("__MIKOMAI_DEBUG__{core}"), false);
        trace.stream("回答\n次の行", true);
        trace.finish().unwrap();
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        let forwarded: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(
            forwarded["payload"],
            serde_json::from_str::<Value>(&core).unwrap()["payload"]
        );
        assert_eq!(forwarded["seq"], 1);
        assert_eq!(forwarded["version"], 1);
        let chunk: Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(chunk["kind"], "core_stream");
        assert_eq!(
            chunk["payload"],
            json!({"text": "回答\n次の行", "done": true})
        );
        assert!(chrono::DateTime::parse_from_rfc3339(chunk["timestamp"].as_str().unwrap()).is_ok());
    }

    #[test]
    fn disabled_trace_does_not_write() {
        let mut output = Vec::new();
        let mut trace = Trace::new(&mut output, false);
        trace.stream("answer", true);
        trace.stream("__MIKOMAI_DEBUG__{}", false);
        trace.finish().unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn closed_stdout_is_reported_without_panicking_in_callback() {
        let mut trace = Trace::new(std::io::Cursor::new(&mut [0u8; 0][..]), true);
        trace.stream("answer", false);
        assert!(trace.finish().unwrap_err().contains("stdout write failed"));
    }
}
