//! Persistent JSONL worker. Only an explicit/bundled executable is launched.
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::{watch, Mutex},
};
use zeroize::Zeroize;

struct Process {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}
pub struct DeviceWorker {
    executable: PathBuf,
    process: Mutex<Option<Process>>,
}
#[derive(Clone, Debug)]
pub struct WorkerResult {
    pub status: String,
    pub payload: Value,
}
impl DeviceWorker {
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            executable: path.into(),
            process: Mutex::new(None),
        }
    }
    pub async fn execute(
        &self,
        mut request: Value,
        secrets: &[String],
        mut cancel: watch::Receiver<bool>,
    ) -> Result<WorkerResult, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let timeout = request["timeout"]
            .as_f64()
            .filter(|n| n.is_finite() && *n > 0. && *n <= 300.)
            .ok_or("invalid worker timeout")?;
        request["id"] = json!(id);
        request["version"] = json!(1);
        let mut slot = tokio::select! { guard=self.process.lock()=>guard, _=cancel.changed()=>return Ok(WorkerResult{status:"cancelled".into(),payload:Value::Null}) };
        if *cancel.borrow() {
            return Ok(WorkerResult {
                status: "cancelled".into(),
                payload: Value::Null,
            });
        }
        if slot.is_none() {
            if !self.executable.is_absolute() {
                return Err("worker executable must be an absolute path".into());
            }
            let mut child = Command::new(&self.executable)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .spawn()
                .map_err(|_| "could not start bundled device worker".to_string())?;
            let input = child.stdin.take().ok_or("worker stdin unavailable")?;
            let output = BufReader::new(child.stdout.take().ok_or("worker stdout unavailable")?);
            *slot = Some(Process {
                child,
                input,
                output,
            });
        }
        let process = slot.as_mut().unwrap();
        let mut wire = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
        wire.push(b'\n');
        let change = matches!(request["op"].as_str(), Some("config" | "console"));
        let dispatched = process.input.write_all(&wire).await;
        wire.zeroize();
        crate::secrets::wipe(&mut request);
        drop(request);
        if dispatched.is_err() {
            let _ = process.child.kill().await;
            *slot = None;
            return Ok(WorkerResult {
                status: if change { "unknown" } else { "failed" }.into(),
                payload: json!({"error":"worker input failed"}),
            });
        }
        let read = async {
            let mut sent = false;
            loop {
                let mut bytes = Vec::new();
                // read_until is bounded with take so an untrusted worker cannot allocate indefinitely.
                use tokio::io::AsyncReadExt;
                let n = (&mut process.output)
                    .take(8 * 1024 * 1024 + 1)
                    .read_until(b'\n', &mut bytes)
                    .await
                    .map_err(|_| "worker read failed")?;
                if n == 0 || n > 8 * 1024 * 1024 || bytes.last() != Some(&b'\n') {
                    return Err("worker output unavailable or oversized");
                }
                let text = String::from_utf8(bytes).map_err(|_| "worker output not UTF-8")?;
                let response: Value =
                    serde_json::from_str(&text).map_err(|_| "invalid worker JSON")?;
                if response["version"] != 1 || response["id"] != id {
                    return Err("worker protocol mismatch");
                }
                if response["phase"] == "sending" {
                    sent = true;
                    continue;
                }
                let mut status = response["status"]
                    .as_str()
                    .filter(|s| matches!(*s, "completed" | "failed" | "cancelled"))
                    .ok_or("invalid worker status")?
                    .to_owned();
                if change && sent && status == "failed" {
                    status = "unknown".into();
                }
                let mut payload = response
                    .get("payload")
                    .cloned()
                    .unwrap_or_else(|| json!({"error":response["error"]}));
                redact_payload(&mut payload, secrets);
                return Ok(WorkerResult { status, payload });
            }
        };
        let outcome = tokio::select! {
            response=tokio::time::timeout(Duration::from_secs_f64(timeout),read)=>match response {Ok(v)=>v,Err(_)=>Err("worker deadline exceeded")},
            _=cancel.changed()=>Err("worker cancelled"),
        };
        match outcome {
            Ok(result) => Ok(result),
            Err(error) => {
                let interrupt = json!({"version":1,"id":id,"op":"cancel"}).to_string() + "\n";
                let _ = tokio::time::timeout(
                    Duration::from_millis(100),
                    process.input.write_all(interrupt.as_bytes()),
                )
                .await;
                tokio::time::sleep(Duration::from_millis(100)).await;
                let _ = process.child.kill().await;
                let _ = process.child.wait().await;
                *slot = None;
                Ok(WorkerResult {
                    status: if change {
                        "unknown"
                    } else if *cancel.borrow() {
                        "cancelled"
                    } else {
                        "failed"
                    }
                    .into(),
                    payload: json!({"error":error}),
                })
            }
        }
    }
}

fn redact_payload(value: &mut Value, secrets: &[String]) {
    match value {
        Value::String(text) => {
            for secret in secrets.iter().filter(|s| !s.is_empty()) {
                *text = text.replace(secret, "[redacted]");
            }
            *text = text
                .lines()
                .map(|line| {
                    if line.split_whitespace().any(|word| {
                        matches!(
                            word.to_ascii_lowercase().as_str(),
                            "password" | "secret" | "community"
                        )
                    }) {
                        "[redacted configuration]"
                    } else {
                        line
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
        }
        Value::Array(values) => values.iter_mut().for_each(|v| redact_payload(v, secrets)),
        Value::Object(values) => values.values_mut().for_each(|v| redact_payload(v, secrets)),
        _ => {}
    }
}

pub fn bundled_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("MIKOMAI_DEVICE_WORKER") {
        return Ok(PathBuf::from(path));
    }
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let parent = executable.parent().ok_or("executable path unavailable")?;
    let candidates = [
        parent.join("../Resources/mikomai-device-worker-macos-arm64"),
        parent.join("mikomai-device-worker-macos-arm64"),
    ];
    candidates.into_iter().find(|p|p.is_file()).ok_or_else(||"bundled device worker is missing; configure an explicit absolute MIKOMAI_DEVICE_WORKER path for development".into())
}

pub struct WorkerPool {
    workers: Vec<Arc<DeviceWorker>>,
    next: std::sync::atomic::AtomicUsize,
}
impl WorkerPool {
    pub fn new(path: &Path) -> Self {
        Self {
            workers: (0..5).map(|_| Arc::new(DeviceWorker::at(path))).collect(),
            next: std::sync::atomic::AtomicUsize::new(0),
        }
    }
    pub fn next(&self) -> Arc<DeviceWorker> {
        self.workers
            [self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % self.workers.len()]
        .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn persistent_worker_redacts_bounds_restarts_and_never_retries_changes() {
        use std::os::unix::fs::PermissionsExt;
        let path =
            std::env::temp_dir().join(format!("mikomai-worker-fixture-{}", uuid::Uuid::new_v4()));
        std::fs::write(&path,r#"#!/usr/bin/python3
import sys,json,time,os
for line in sys.stdin:
 r=json.loads(line)
 if r['op']=='cancel': continue
 if r.get('mode')=='hang':
  print(json.dumps({'version':1,'id':r['id'],'phase':'sending'}),flush=True)
  time.sleep(5)
 elif r.get('mode')=='crash': sys.exit(7)
 else: print(json.dumps({'version':1,'id':r['id'],'status':'completed','payload':{'pid':os.getpid(),'output':r.get('credentials',{}).get('password','ok')}}),flush=True)
"#).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let worker = DeviceWorker::at(&path);
        let (_keep, cancel) = watch::channel(false);
        let first = worker
            .execute(
                json!({"op":"show","timeout":2,"credentials":{"password":"a\"日本語\nsecret"}}),
                &["a\"日本語\nsecret".into()],
                cancel.clone(),
            )
            .await
            .unwrap();
        assert_eq!(first.status, "completed");
        assert_eq!(first.payload["output"], "[redacted]");
        let second = worker
            .execute(json!({"op":"show","timeout":2}), &[], cancel.clone())
            .await
            .unwrap();
        assert_eq!(first.payload["pid"], second.payload["pid"]);
        let timeout = worker
            .execute(
                json!({"op":"config","mode":"hang","timeout":0.1}),
                &[],
                cancel.clone(),
            )
            .await
            .unwrap();
        assert_eq!(timeout.status, "unknown");
        let restarted = worker
            .execute(json!({"op":"show","timeout":2}), &[], cancel.clone())
            .await
            .unwrap();
        assert_ne!(first.payload["pid"], restarted.payload["pid"]);
        let crashed = worker
            .execute(
                json!({"op":"config","mode":"crash","timeout":2}),
                &[],
                cancel.clone(),
            )
            .await
            .unwrap();
        assert_eq!(crashed.status, "unknown");
        let (stop, c) = watch::channel(false);
        stop.send(true).unwrap();
        assert_eq!(
            worker
                .execute(json!({"op":"show","timeout":2}), &[], c)
                .await
                .unwrap()
                .status,
            "cancelled"
        );
        std::fs::remove_file(path).unwrap();
    }
}
