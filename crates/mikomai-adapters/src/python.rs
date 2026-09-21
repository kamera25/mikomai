//! Optional Python helpers with one process contract and no cwd assumptions.
pub trait PythonRunner: Send + Sync {
    fn run_json(&self, program: &str, input: &[u8]) -> Result<Vec<u8>, String>;
}

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct PythonAdapter {
    pub interpreter: PathBuf,
    pub working_dir: Option<PathBuf>,
    pub timeout: Duration,
}
impl PythonAdapter {
    pub fn new(interpreter: impl Into<PathBuf>) -> Self {
        Self {
            interpreter: interpreter.into(),
            working_dir: None,
            timeout: Duration::from_secs(30),
        }
    }
    pub fn with_working_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.working_dir = Some(path.as_ref().to_path_buf());
        self
    }
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}
impl PythonRunner for PythonAdapter {
    fn run_json(&self, program: &str, input: &[u8]) -> Result<Vec<u8>, String> {
        let mut command = Command::new(&self.interpreter);
        command
            .arg(program)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(directory) = &self.working_dir {
            command.current_dir(directory);
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("failed to start python adapter: {error}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            std::io::Write::write_all(&mut stdin, input).map_err(|error| error.to_string())?;
        }
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
                let output = child
                    .wait_with_output()
                    .map_err(|error| error.to_string())?;
                if !status.success() {
                    return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
                }
                return Ok(output.stdout);
            }
            if started.elapsed() >= self.timeout {
                let _ = child.kill();
                return Err(format!("python adapter timed out after {:?}", self.timeout));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
