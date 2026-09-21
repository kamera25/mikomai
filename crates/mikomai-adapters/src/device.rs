//! Network device transports (MCP/Netmiko/serial) are adapters, never domain dependencies.
pub trait DeviceTransport: Send + Sync {
    fn observe(&self, target: &str, request: &str) -> Result<String, String>;
    fn apply(&self, target: &str, command: &str) -> Result<String, String>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSummary {
    pub hostname: String,
    #[serde(default)]
    pub ip: Option<String>,
    #[serde(rename = "type", alias = "connectionType", default)]
    pub connection_type: Option<String>,
    #[serde(default)]
    pub device_type: Option<String>,
}

/// Headless registry adapter. Secrets are intentionally not deserialized;
/// connection credentials remain owned by the desktop keyring layer.
pub struct JsonDeviceRegistry {
    path: std::path::PathBuf,
}
impl JsonDeviceRegistry {
    pub fn at(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn from_env() -> Self {
        let path = std::env::var_os("MIKOMAI_CONNECTIONS_FILE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("connections.json"));
        Self::at(path)
    }

    pub fn list(&self) -> Result<Vec<DeviceSummary>, String> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let bytes = std::fs::read(&self.path).map_err(|error| error.to_string())?;
        serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "failed to parse device registry {}: {error}",
                self.path.display()
            )
        })
    }
}

pub struct DeviceToolExecutor<T>(pub T);
pub type DeviceAdapter<T> = DeviceToolExecutor<T>;
impl<T: DeviceTransport> mikomai_core::port::ToolExecutorPort for DeviceToolExecutor<T> {
    fn execute<'a>(
        &'a self,
        _task_id: uuid::Uuid,
        tool: &'a str,
        target: Option<&'a str>,
        args: &'a serde_json::Value,
    ) -> mikomai_core::port::PortFuture<'a, mikomai_core::port::ToolResult> {
        Box::pin(async move {
            let target = target.ok_or_else(|| "device target is required".to_string())?;
            let request = args
                .get("command")
                .and_then(|value| value.as_str())
                .unwrap_or(tool);
            let output = if ["network_config", "configure", "write_config"].contains(&tool) {
                self.0.apply(target, request)?
            } else {
                self.0.observe(target, request)?
            };
            Ok(mikomai_core::port::ToolResult {
                success: true,
                output,
            })
        })
    }
}

pub struct FnDeviceTransport<O, A> {
    pub observe_fn: O,
    pub apply_fn: A,
}
impl<O, A> DeviceTransport for FnDeviceTransport<O, A>
where
    O: Fn(&str, &str) -> Result<String, String> + Send + Sync,
    A: Fn(&str, &str) -> Result<String, String> + Send + Sync,
{
    fn observe(&self, target: &str, request: &str) -> Result<String, String> {
        (self.observe_fn)(target, request)
    }
    fn apply(&self, target: &str, command: &str) -> Result<String, String> {
        (self.apply_fn)(target, command)
    }
}
