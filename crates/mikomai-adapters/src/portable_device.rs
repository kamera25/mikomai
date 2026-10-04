//! Portable, read-only device execution boundary for native clients.
//!
//! This module deliberately knows nothing about a keychain or a
//! particular SSH library. The caller supplies credentials per invocation;
//! the registry and tool descriptors contain only non-secret target metadata.
use mikomai_core::port::ToolResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use zeroize::Zeroize;

pub use mikomai_core::agent::RegisteredDevice;

/// Credentials are deliberately neither serializable nor cloneable. Keep the
/// keychain boundary at the caller and pass these only to a transport call.
pub struct DeviceCredentials {
    pub username: String,
    pub password: Option<String>,
    pub enable_password: Option<String>,
    pub private_key: Option<String>,
    pub passphrase: Option<String>,
}

impl fmt::Debug for DeviceCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceCredentials")
            .field("username", &"[redacted]")
            .field("password", &self.password.as_ref().map(|_| "[redacted]"))
            .field(
                "enable_password",
                &self.enable_password.as_ref().map(|_| "[redacted]"),
            )
            .field(
                "private_key",
                &self.private_key.as_ref().map(|_| "[redacted]"),
            )
            .field(
                "passphrase",
                &self.passphrase.as_ref().map(|_| "[redacted]"),
            )
            .finish()
    }
}

impl Drop for DeviceCredentials {
    fn drop(&mut self) {
        // Best-effort buffer clearing; callers should still minimize lifetime.
        self.username.zeroize();
        if let Some(value) = &mut self.password {
            value.zeroize();
        }
        if let Some(value) = &mut self.enable_password {
            value.zeroize();
        }
        if let Some(value) = &mut self.private_key {
            value.zeroize();
        }
        if let Some(value) = &mut self.passphrase {
            value.zeroize();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOnlyDeviceTool {
    GetState,
    FetchConfig,
    FetchRouting,
    FetchArp,
    NetworkShow,
    SelfNetworkPing,
    SelfNetworkTraceroute,
    SelfNetworkTestConnection,
    SelfNetworkRoute,
    NetworkGetIpInfo,
    NetworkListSerialPorts,
    KnowledgeSearch,
    NetworkPacketAnalyze,
    NetworkPacketPrepare,
    NetworkPacketSafety,
    SelfNetworkNwdiag,
    ValidateCiscoConfig,
    ConvertCiscoConfig,
}

impl ReadOnlyDeviceTool {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GetState => "get_state",
            Self::FetchConfig => "fetch_config",
            Self::FetchRouting => "fetch_routing",
            Self::FetchArp => "fetch_arp",
            Self::NetworkShow => "network_show",
            Self::SelfNetworkPing => "self_network_ping",
            Self::SelfNetworkTraceroute => "self_network_traceroute",
            Self::SelfNetworkTestConnection => "self_network_test_connection",
            Self::SelfNetworkRoute => "self_network_route",
            Self::NetworkGetIpInfo => "network_get_ip_info",
            Self::NetworkListSerialPorts => "network_list_serial_ports",
            Self::KnowledgeSearch => "query_nw_db",
            Self::NetworkPacketAnalyze => "network_packet_analyze",
            Self::NetworkPacketPrepare => "network_packet_prepare",
            Self::NetworkPacketSafety => "network_packet_safety",
            Self::SelfNetworkNwdiag => "self_network_nwdiag",
            Self::ValidateCiscoConfig => "validate_cisco_config",
            Self::ConvertCiscoConfig => "convert_cisco_config",
        }
    }

    pub fn parse(tool: &str) -> Result<Self, String> {
        match tool {
            "get_state" => Ok(Self::GetState),
            "fetch_config" => Ok(Self::FetchConfig),
            "fetch_routing" => Ok(Self::FetchRouting),
            "fetch_arp" => Ok(Self::FetchArp),
            "network_show" => Ok(Self::NetworkShow),
            "self_network_ping" => Ok(Self::SelfNetworkPing),
            "self_network_traceroute" => Ok(Self::SelfNetworkTraceroute),
            "self_network_test_connection" | "self_network_test_net_connection" => {
                Ok(Self::SelfNetworkTestConnection)
            }
            "self_network_route" => Ok(Self::SelfNetworkRoute),
            "network_get_ip_info" => Ok(Self::NetworkGetIpInfo),
            "network_list_serial_ports" => Ok(Self::NetworkListSerialPorts),
            "query_nw_db" | "network_query_nw_db" | "query_rag" => Ok(Self::KnowledgeSearch),
            "network_packet_analyze" => Ok(Self::NetworkPacketAnalyze),
            "network_packet_prepare" => Ok(Self::NetworkPacketPrepare),
            "network_packet_safety" => Ok(Self::NetworkPacketSafety),
            "self_network_nwdiag" => Ok(Self::SelfNetworkNwdiag),
            "validate_cisco_config" => Ok(Self::ValidateCiscoConfig),
            "convert_cisco_config" => Ok(Self::ConvertCiscoConfig),
            _ => Err(format!(
                "tool is not available in the portable read-only registry: {tool}"
            )),
        }
    }
}

/// Public metadata for a tool. It never contains target credentials.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReadOnlyToolDescriptor {
    pub id: String,
    pub read_only: bool,
}

pub trait CredentialedReadOnlyTransport: Send + Sync {
    fn execute_read_only(
        &self,
        target: &RegisteredDevice,
        credentials: &DeviceCredentials,
        tool: ReadOnlyDeviceTool,
        args: &Value,
    ) -> Result<String, String>;
}

/// An allow-listed inventory. Call `from_json` with only public device fields;
/// unknown JSON fields (including legacy embedded credentials) are discarded.
#[derive(Debug, Clone, Default)]
pub struct ReadOnlyToolRegistry {
    devices: Vec<RegisteredDevice>,
}

impl ReadOnlyToolRegistry {
    pub fn new(devices: Vec<RegisteredDevice>) -> Result<Self, String> {
        if devices
            .iter()
            .any(|device| device.hostname.trim().is_empty())
        {
            return Err("registered device hostname cannot be empty".into());
        }
        Ok(Self { devices })
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let devices: Vec<RegisteredDevice> = serde_json::from_str(json)
            .map_err(|error| format!("invalid device registry: {error}"))?;
        Self::new(devices)
    }

    pub fn devices(&self) -> &[RegisteredDevice] {
        &self.devices
    }

    pub fn tools(&self) -> Vec<ReadOnlyToolDescriptor> {
        let mut tools: Vec<_> = [
            ReadOnlyDeviceTool::GetState,
            ReadOnlyDeviceTool::FetchConfig,
            ReadOnlyDeviceTool::FetchRouting,
            ReadOnlyDeviceTool::FetchArp,
            ReadOnlyDeviceTool::NetworkShow,
            ReadOnlyDeviceTool::SelfNetworkPing,
            ReadOnlyDeviceTool::SelfNetworkTraceroute,
            ReadOnlyDeviceTool::SelfNetworkTestConnection,
            ReadOnlyDeviceTool::SelfNetworkRoute,
            ReadOnlyDeviceTool::NetworkGetIpInfo,
            ReadOnlyDeviceTool::NetworkListSerialPorts,
            ReadOnlyDeviceTool::KnowledgeSearch,
            ReadOnlyDeviceTool::NetworkPacketAnalyze,
            ReadOnlyDeviceTool::NetworkPacketPrepare,
            ReadOnlyDeviceTool::NetworkPacketSafety,
            ReadOnlyDeviceTool::SelfNetworkNwdiag,
            ReadOnlyDeviceTool::ValidateCiscoConfig,
            ReadOnlyDeviceTool::ConvertCiscoConfig,
        ]
        .into_iter()
        .map(|tool| ReadOnlyToolDescriptor {
            id: tool.as_str().into(),
            read_only: true,
        })
        .collect();
        tools.extend(
            [
                "network_query_nw_db",
                "query_rag",
                "self_network_test_net_connection",
            ]
            .into_iter()
            .map(|id| ReadOnlyToolDescriptor {
                id: id.into(),
                read_only: true,
            }),
        );
        tools
    }

    /// Executes a single read-only device tool against a registered device.
    /// Writes and unregistered destinations are rejected before transport use.
    pub fn execute<T: CredentialedReadOnlyTransport>(
        &self,
        transport: &T,
        tool_id: &str,
        target_id: &str,
        args: &Value,
        credentials: &DeviceCredentials,
    ) -> Result<ToolResult, String> {
        let tool = ReadOnlyDeviceTool::parse(tool_id)?;
        let mut resolved_args = args.clone();
        if matches!(tool, ReadOnlyDeviceTool::SelfNetworkPing | ReadOnlyDeviceTool::SelfNetworkTraceroute | ReadOnlyDeviceTool::SelfNetworkTestConnection) {
            if let Some(host) = args.get("host").and_then(Value::as_str) {
                let matches = self.devices.iter().filter(|device| {
                    device.hostname.trim().eq_ignore_ascii_case(host.trim()) || device.id.as_deref() == Some(host.trim())
                }).collect::<Vec<_>>();
                if matches.len() > 1 { return Err("registered diagnostic target is ambiguous".into()); }
                if let Some(device) = matches.first() {
                    let ip = device.ip.as_deref().filter(|ip| !ip.trim().is_empty())
                        .ok_or("registered diagnostic target has no IP address")?;
                    let ip = ip.trim().parse::<std::net::IpAddr>()
                        .map_err(|_| "registered diagnostic target has an invalid IP address; DNS lookup is disabled for registered devices")?;
                    resolved_args["host"] = ip.to_string().into();
                }
            }
        }
        let args = &resolved_args;
        validate_tool_args(tool, args)?;
        let local_ndp = tool == ReadOnlyDeviceTool::GetState && args["resource"] == "ndp";
        if local_ndp && (target_id != "localhost" || args["device"].as_str().is_some_and(|device| device != "localhost")) {
            return Err("NDP observation is supported only on localhost".into());
        }
        let target = if is_local_tool_id(tool_id) || local_ndp {
            RegisteredDevice {
                id: None,
                hostname: target_id.to_owned(),
                ip: None,
                device_type: None,
            }
        } else {
            self.devices
                .iter()
                .find(|device| {
                    device.id.as_deref() == Some(target_id)
                        || device.hostname == target_id
                        || device.ip.as_deref() == Some(target_id)
                })
                .cloned()
                .ok_or_else(|| "device target is not registered".to_string())?
        };
        let output = transport.execute_read_only(&target, credentials, tool, args)?;
        Ok(ToolResult {
            success: true,
            output,
        })
    }
}

fn validate_tool_args(tool: ReadOnlyDeviceTool, args: &Value) -> Result<(), String> {
    if !args.is_object() {
        return Err("tool arguments must be a JSON object".into());
    }
    if matches!(
        tool,
        ReadOnlyDeviceTool::SelfNetworkPing
            | ReadOnlyDeviceTool::SelfNetworkTraceroute
            | ReadOnlyDeviceTool::SelfNetworkTestConnection
    ) {
        let host = args
            .get("host")
            .and_then(Value::as_str)
            .ok_or_else(|| "network diagnostic requires a host".to_string())?;
        if host.is_empty()
            || host.len() > 255
            || host.starts_with('-')
            || !host
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ".-:%".contains(ch))
        {
            return Err("host must be a DNS name or IP address".into());
        }
        if tool == ReadOnlyDeviceTool::SelfNetworkTestConnection {
            if args.get("protocol").is_some_and(|p| p.as_str().is_none_or(|p| !p.eq_ignore_ascii_case("tcp"))) {
                return Err("port check supports TCP only".into());
            }
            let port = args
                .get("port")
                .and_then(Value::as_u64)
                .ok_or_else(|| "connection test requires a port".to_string())?;
            if !(1..=65535).contains(&port) {
                return Err("port must be between 1 and 65535".into());
            }
        }
    } else if tool == ReadOnlyDeviceTool::NetworkShow {
        let command = args
            .get("command")
            .and_then(Value::as_str)
            .ok_or_else(|| "network_show requires a command".to_string())?;
        let normalized = command.trim();
        let lower = normalized.to_ascii_lowercase();
        if !lower.starts_with("show ")
            || normalized.len() > 256
            || normalized
                .chars()
                .any(|ch| ch.is_control() || ";|&$`()<>\\\"'".contains(ch))
        {
            return Err("network_show accepts one bounded read-only show command".into());
        }
    } else if tool == ReadOnlyDeviceTool::NetworkPacketAnalyze {
        let frame = args
            .get("frame_hex")
            .or_else(|| args.get("frameHex"))
            .and_then(Value::as_str)
            .ok_or_else(|| "network_packet_analyze requires frame_hex".to_string())?;
        if frame.len() > 3_072 {
            return Err("frame_hex exceeds the portable packet-analysis limit".into());
        }
    } else if matches!(
        tool,
        ReadOnlyDeviceTool::NetworkPacketPrepare | ReadOnlyDeviceTool::NetworkPacketSafety
    ) {
        if serde_json::to_vec(args).map_or(usize::MAX, |encoded| encoded.len()) > 8_192 {
            return Err("packet request exceeds the portable packet-worker limit".into());
        }
    } else if tool == ReadOnlyDeviceTool::SelfNetworkNwdiag {
        if args
            .get("schema")
            .or_else(|| args.get("nwdiag"))
            .and_then(Value::as_str)
            .is_none_or(|schema| schema.is_empty() || schema.len() > 128 * 1024)
        {
            return Err("nwdiag schema must contain 1–131,072 characters".into());
        }
    } else if matches!(
        tool,
        ReadOnlyDeviceTool::ValidateCiscoConfig | ReadOnlyDeviceTool::ConvertCiscoConfig
    ) {
        if args
            .get("config")
            .and_then(Value::as_str)
            .is_none_or(|config| config.is_empty() || config.len() > 512 * 1024)
        {
            return Err("config must contain 1–524,288 characters".into());
        }
        if tool == ReadOnlyDeviceTool::ConvertCiscoConfig {
            let vendor = args
                .get("target_vendor")
                .or_else(|| args.get("targetVendor"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !matches!(vendor, "arista" | "juniper") {
                return Err("target_vendor must be arista or juniper".into());
            }
        }
    } else if args.get("command").is_some() {
        return Err("command override is not accepted for this tool".into());
    }
    Ok(())
}

pub fn is_local_tool_id(tool_id: &str) -> bool {
    let Ok(tool) = ReadOnlyDeviceTool::parse(tool_id) else {
        return false;
    };
    matches!(
        tool,
        ReadOnlyDeviceTool::SelfNetworkPing
            | ReadOnlyDeviceTool::SelfNetworkTraceroute
            | ReadOnlyDeviceTool::SelfNetworkTestConnection
            | ReadOnlyDeviceTool::SelfNetworkRoute
            | ReadOnlyDeviceTool::NetworkGetIpInfo
            | ReadOnlyDeviceTool::NetworkListSerialPorts
            | ReadOnlyDeviceTool::KnowledgeSearch
            | ReadOnlyDeviceTool::NetworkPacketAnalyze
            | ReadOnlyDeviceTool::NetworkPacketPrepare
            | ReadOnlyDeviceTool::NetworkPacketSafety
            | ReadOnlyDeviceTool::SelfNetworkNwdiag
            | ReadOnlyDeviceTool::ValidateCiscoConfig
            | ReadOnlyDeviceTool::ConvertCiscoConfig
    )
}

#[cfg(test)]
mod tests {

    #[test]
    fn ndp_accepts_localhost_without_registration_and_rejects_remote_targets() {
        struct Capture(std::sync::Mutex<Vec<String>>);
        impl CredentialedReadOnlyTransport for Capture {
            fn execute_read_only(&self, target: &RegisteredDevice, _: &DeviceCredentials, _: ReadOnlyDeviceTool, _: &Value) -> Result<String, String> {
                self.0.lock().unwrap().push(target.hostname.clone()); Ok("observed".into())
            }
        }
        let registry = ReadOnlyToolRegistry::from_json(r#"[{"hostname":"router"}]"#).unwrap();
        let transport = Capture(std::sync::Mutex::new(vec![]));
        let args = serde_json::json!({"resource":"ndp","device":"localhost"});
        registry.execute(&transport, "get_state", "localhost", &args, &creds()).unwrap();
        assert!(registry.execute(&transport, "get_state", "router", &args, &creds()).is_err());
        assert!(registry.execute(&transport, "get_state", "localhost", &serde_json::json!({"resource":"ndp","device":"router"}), &creds()).is_err());
        assert_eq!(*transport.0.lock().unwrap(), vec!["localhost"]);
    }

    #[test]
    fn registered_hosts_take_priority_for_all_local_network_probes() {
        struct Capture(std::sync::Mutex<Vec<Value>>);
        impl CredentialedReadOnlyTransport for Capture {
            fn execute_read_only(&self, _: &RegisteredDevice, _: &DeviceCredentials, _: ReadOnlyDeviceTool, args: &Value) -> Result<String, String> {
                self.0.lock().unwrap().push(args.clone()); Ok("observed".into())
            }
        }
        let registry = ReadOnlyToolRegistry::from_json(r#"[{"id":"gateway-id","hostname":"NakaokuGW","ip":"192.168.50.1"}]"#).unwrap();
        let transport = Capture(std::sync::Mutex::new(vec![]));
        for tool in ["self_network_ping", "self_network_traceroute", "self_network_test_connection", "self_network_test_net_connection"] {
            for host in ["NakaokuGW", "nakaokugw", "gateway-id"] {
                registry.execute(&transport, tool, "localhost", &serde_json::json!({"host":host,"port":22}), &creds()).unwrap();
                assert_eq!(transport.0.lock().unwrap().last().unwrap()["host"], "192.168.50.1");
            }
        }
        registry.execute(&transport, "self_network_ping", "localhost", &serde_json::json!({"host":"unregistered.example"}), &creds()).unwrap();
        assert_eq!(transport.0.lock().unwrap().last().unwrap()["host"], "unregistered.example");
        let missing = ReadOnlyToolRegistry::from_json(r#"[{"hostname":"NakaokuGW"}]"#).unwrap();
        assert!(missing.execute(&transport, "self_network_ping", "localhost", &serde_json::json!({"host":"NakaokuGW"}), &creds()).is_err());
        let ambiguous = ReadOnlyToolRegistry::from_json(r#"[{"hostname":"NakaokuGW","ip":"192.0.2.1"},{"hostname":"nakaokugw","ip":"192.0.2.2"}]"#).unwrap();
        assert!(ambiguous.execute(&transport, "self_network_ping", "localhost", &serde_json::json!({"host":"NakaokuGW"}), &creds()).is_err());
        assert_eq!(transport.0.lock().unwrap().len(), 13);
    }

    #[test]
    fn port_check_resolves_inventory_names_and_rejects_udp_before_transport() {
        struct Capture(std::sync::Mutex<Vec<Value>>);
        impl CredentialedReadOnlyTransport for Capture {
            fn execute_read_only(&self, _: &RegisteredDevice, _: &DeviceCredentials, _: ReadOnlyDeviceTool, args: &Value) -> Result<String, String> {
                self.0.lock().unwrap().push(args.clone()); Ok("observed".into())
            }
        }
        let registry = ReadOnlyToolRegistry::from_json(r#"[{"hostname":"NakaokuGW","ip":"192.168.50.1"}]"#).unwrap();
        let transport = Capture(std::sync::Mutex::new(vec![]));
        registry.execute(&transport, "self_network_test_connection", "localhost", &serde_json::json!({"host":"NakaokuGW","port":22}), &creds()).unwrap();
        assert_eq!(transport.0.lock().unwrap()[0]["host"], "192.168.50.1");
        for args in [serde_json::json!({"host":"NakaokuGW","port":53,"protocol":"udp"}), serde_json::json!({"host":"NakaokuGW","port":65536})] {
            assert!(registry.execute(&transport, "self_network_test_connection", "localhost", &args, &creds()).is_err());
        }
        assert_eq!(transport.0.lock().unwrap().len(), 1);
    }

    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Transport(AtomicUsize);
    impl CredentialedReadOnlyTransport for Transport {
        fn execute_read_only(
            &self,
            target: &RegisteredDevice,
            credentials: &DeviceCredentials,
            tool: ReadOnlyDeviceTool,
            _args: &Value,
        ) -> Result<String, String> {
            assert_eq!(target.hostname, "router-a");
            assert_eq!(credentials.password.as_deref(), Some("secret"));
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(tool.as_str().to_string())
        }
    }
    fn creds() -> DeviceCredentials {
        DeviceCredentials {
            username: "operator".into(),
            password: Some("secret".into()),
            enable_password: None,
            private_key: None,
            passphrase: None,
        }
    }
    fn registry() -> ReadOnlyToolRegistry {
        ReadOnlyToolRegistry::new(vec![RegisteredDevice {
            id: Some("id-1".into()),
            hostname: "router-a".into(),
            ip: Some("192.0.2.1".into()),
            device_type: Some("cisco_ios".into()),
        }])
        .unwrap()
    }

    #[test]
    fn registry_and_tool_metadata_never_contain_secrets() {
        let registry = ReadOnlyToolRegistry::from_json(
            r#"[{"id":"r1","hostname":"r","ip":"192.0.2.1","password":"must-not-leak"}]"#,
        )
        .unwrap();
        let json = serde_json::to_string(&registry.devices()).unwrap();
        assert!(!json.contains("must-not-leak"));
        assert!(format!("{:?}", creds()).contains("[redacted]"));
        assert!(registry.tools().iter().all(|tool| tool.read_only));
    }

    #[test]
    fn only_registered_targets_and_allow_listed_tools_reach_transport() {
        let transport = Transport(AtomicUsize::new(0));
        let args = serde_json::json!({"command":"show version"});
        assert!(registry()
            .execute(&transport, "network_show", "id-1", &args, &creds())
            .is_ok());
        assert!(registry()
            .execute(&transport, "network_config", "id-1", &args, &creds())
            .is_err());
        assert!(registry()
            .execute(
                &transport,
                "fetch_config",
                "203.0.113.8",
                &serde_json::json!({}),
                &creds()
            )
            .is_err());
        assert_eq!(transport.0.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn show_commands_cannot_smuggle_writes_or_shell_chains() {
        let transport = Transport(AtomicUsize::new(0));
        for command in [
            "configure terminal",
            "show version; reload",
            "show version\\nreload",
            "show version | include x",
        ] {
            assert!(registry()
                .execute(
                    &transport,
                    "network_show",
                    "id-1",
                    &serde_json::json!({"command":command}),
                    &creds()
                )
                .is_err());
        }
        assert_eq!(transport.0.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn local_diagnostics_are_available_without_registered_devices_and_validate_arguments() {
        struct Local(AtomicUsize);
        impl CredentialedReadOnlyTransport for Local {
            fn execute_read_only(
                &self,
                target: &RegisteredDevice,
                _: &DeviceCredentials,
                tool: ReadOnlyDeviceTool,
                _: &Value,
            ) -> Result<String, String> {
                assert_eq!(target.hostname, "localhost");
                assert_eq!(tool, ReadOnlyDeviceTool::SelfNetworkPing);
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok("ping accepted".into())
            }
        }
        let registry = ReadOnlyToolRegistry::default();
        let local = Local(AtomicUsize::new(0));
        let result = registry
            .execute(
                &local,
                "self_network_ping",
                "localhost",
                &serde_json::json!({"host":"192.0.2.1","count":2}),
                &creds(),
            )
            .unwrap();
        assert!(result.success);
        assert!(registry
            .execute(
                &local,
                "self_network_ping",
                "localhost",
                &serde_json::json!({"host":"-f"}),
                &creds()
            )
            .is_err());
        assert_eq!(local.0.load(Ordering::SeqCst), 1);
        assert!(registry
            .tools()
            .iter()
            .any(|tool| tool.id == "self_network_route"));
    }
}
