//! Optional Python helpers with one process contract and no cwd assumptions.
pub trait PythonRunner: Send + Sync { fn run_json(&self, program: &str, input: &[u8]) -> Result<Vec<u8>, String>; }
