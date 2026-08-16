use crate::connections::{resolve_host_with_mcp, Port};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::LazyLock;
use tokio::time::{timeout, Duration, Instant};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TestConnectionParams
{
    pub host: Option<String>,
    pub device: Option<String>,
    pub device_name: Option<String>,
    pub ip: Option<String>,
    pub computer_name: Option<String>,
    pub port: Option<Port>,
    pub common_tcp_port: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct CommonTcpPortsYaml
{
    ports: HashMap<String, u16>,
}

static COMMON_TCP_PORTS: LazyLock<HashMap<String, u16>> = LazyLock::new(|| {
    let yaml_str = include_str!("config/common_tcp_ports.yaml");
    let parsed: CommonTcpPortsYaml = serde_yaml::from_str(yaml_str).unwrap_or_else(|e| {
        log::error!("Failed to parse common_tcp_ports.yaml: {}", e);
        CommonTcpPortsYaml {
            ports: HashMap::new(),
        }
    });
    parsed.ports
});

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TestConnectionResult
{
    pub success: bool,
    pub output: String,
    pub computer_name: String,
    pub remote_address: String,
    pub remote_port: Option<Port>,
    pub interface_alias: Option<String>,
    pub source_address: Option<String>,
    pub ping_succeeded: bool,
    pub ping_reply_details: Option<String>,
    pub tcp_test_succeeded: Option<bool>,
    pub latency_ms: Option<u64>,
}

impl From<TestConnectionResult> for crate::network::CommandResult
{
    fn from(res: TestConnectionResult) -> Self
    {
        Self {
            success: res.success,
            output: res.output,
            saved_path: None,
            is_cached: None,
            cache_time: None,
        }
    }
}

pub fn resolve_common_tcp_port(port_str: &str) -> Option<Port>
{
    let key = port_str.trim().to_uppercase();
    if let Some(&port) = COMMON_TCP_PORTS.get(&key)
    {
        Port::try_from(port).ok()
    }
    else
    {
        Port::try_from(port_str).ok()
    }
}

fn get_local_source_info(target_ip: IpAddr, port: u16) -> (Option<String>, Option<String>)
{
    let target_addr = SocketAddr::new(target_ip, port);
    let bind_addr = match target_ip
    {
        IpAddr::V4(_) => "0.0.0.0:0",
        IpAddr::V6(_) => "[::]:0",
    };

    if let Ok(socket) = UdpSocket::bind(bind_addr)
    {
        if socket.connect(target_addr).is_ok()
        {
            if let Ok(local_addr) = socket.local_addr()
            {
                let ip_str = local_addr.ip().to_string();
                return (Some(ip_str), None);
            }
        }
    }
    (None, None)
}

pub async fn network_test_connection_core(
    target_host: String,
    ip_addr: IpAddr,
    port: Option<Port>,
    common_tcp_port: Option<String>,
    timeout_ms: Option<u64>,
) -> Result<TestConnectionResult, String>
{
    let target_port = port.or_else(|| common_tcp_port.as_deref().and_then(resolve_common_tcp_port));

    let timeout_duration = Duration::from_millis(timeout_ms.unwrap_or(2000));
    let (source_address, interface_alias) =
        get_local_source_info(ip_addr, target_port.map(|p| *p).unwrap_or(80));

    // 1. ICMP Ping test
    let ping_res =
        crate::mcp::ping::network_ping_core(ip_addr.to_string(), Some(32), Some(1), None).await;

    let (ping_succeeded, ping_reply_details) = match ping_res
    {
        Ok(res) =>
        {
            if res.success
            {
                let rtt_str = res
                    .output
                    .lines()
                    .find(|line| line.contains("time=") || line.contains("time<"))
                    .map(|line| line.trim().to_string())
                    .unwrap_or_else(|| "Reply received".to_string());
                (true, Some(rtt_str))
            }
            else
            {
                (false, Some("Ping request timed out".to_string()))
            }
        }
        Err(e) => (false, Some(format!("Ping error: {}", e))),
    };

    // 2. TCP Port connection test (if port is requested)
    let (tcp_test_succeeded, tcp_latency_ms) = if let Some(p) = target_port
    {
        let addr = SocketAddr::new(ip_addr, *p);
        let start = Instant::now();
        match timeout(timeout_duration, tokio::net::TcpStream::connect(addr)).await
        {
            Ok(Ok(_stream)) => (Some(true), Some(start.elapsed().as_millis() as u64)),
            Ok(Err(_err)) => (Some(false), None),
            Err(_timeout) => (Some(false), None),
        }
    }
    else
    {
        (None, None)
    };

    let overall_success = match tcp_test_succeeded
    {
        Some(tcp_succ) => tcp_succ,
        None => ping_succeeded,
    };

    // Build readable output report
    let mut output = String::new();
    output.push_str(&format!("{:<23}: {}\n", "ComputerName", target_host));
    output.push_str(&format!("{:<23}: {}\n", "RemoteAddress", ip_addr));

    if let Some(p) = target_port
    {
        output.push_str(&format!("{:<23}: {}\n", "RemotePort", p));
    }
    if let Some(ref iface) = interface_alias
    {
        output.push_str(&format!("{:<23}: {}\n", "InterfaceAlias", iface));
    }
    if let Some(ref src) = source_address
    {
        output.push_str(&format!("{:<23}: {}\n", "SourceAddress", src));
    }
    if let Some(tcp_succ) = tcp_test_succeeded
    {
        output.push_str(&format!(
            "{:<23}: {}\n",
            "TcpTestSucceeded",
            if tcp_succ { "True" } else { "False" }
        ));
    }

    output.push_str(&format!(
        "{:<23}: {}\n",
        "PingSucceeded",
        if ping_succeeded { "True" } else { "False" }
    ));

    if let Some(ref details) = ping_reply_details
    {
        output.push_str(&format!("{:<23}: {}\n", "PingReplyDetails", details));
    }
    if let Some(lat) = tcp_latency_ms
    {
        output.push_str(&format!("{:<23}: {} ms\n", "TcpConnectTime", lat));
    }

    if !overall_success
    {
        if let Some(p) = target_port
        {
            output.push_str(&format!(
                "\nWarning: TCP connection to {}:{} failed or timed out after {} ms.\n",
                ip_addr,
                p,
                timeout_duration.as_millis()
            ));
        }
        else
        {
            output.push_str(&format!(
                "\nWarning: Ping to {} failed or timed out.\n",
                ip_addr
            ));
        }
    }

    Ok(TestConnectionResult {
        success: overall_success,
        output,
        computer_name: target_host,
        remote_address: ip_addr.to_string(),
        remote_port: target_port,
        interface_alias,
        source_address,
        ping_succeeded,
        ping_reply_details,
        tcp_test_succeeded,
        latency_ms: tcp_latency_ms,
    })
}

pub async fn self_network_test_connection_with_params(
    app: tauri::AppHandle,
    params: TestConnectionParams,
) -> Result<TestConnectionResult, String>
{
    let target_host = params
        .host
        .or(params.computer_name)
        .or_else(|| {
            crate::mcp::args::normalize_host_args(
                &app,
                None,
                params.device,
                params.device_name.clone(),
                params.device_name,
                params.ip,
            )
            .ok()
        })
        .or_else(|| {
            if let Ok(settings) = crate::settings::load_settings(app.clone())
            {
                settings.recent_ips.first().cloned()
            }
            else
            {
                None
            }
        })
        .ok_or_else(|| "Target host or computer_name is required".to_string())?;

    let resolved_host = resolve_host_with_mcp(&app, &target_host);
    let app_clone = app.clone();
    let resolved_host_clone = resolved_host.clone();
    let ip_addr = tokio::task::spawn_blocking(move || {
        crate::connections::resolve_host_with_preference(&app_clone, &resolved_host_clone)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    network_test_connection_core(
        target_host,
        ip_addr,
        params.port,
        params.common_tcp_port,
        params.timeout_ms,
    )
    .await
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn self_network_test_connection(
    app: tauri::AppHandle,
    host: Option<String>,
    device: Option<String>,
    deviceName: Option<String>,
    device_name: Option<String>,
    ip: Option<String>,
    computer_name: Option<String>,
    computerName: Option<String>,
    port: Option<Port>,
    common_tcp_port: Option<String>,
    commonTcpPort: Option<String>,
    timeout_ms: Option<u64>,
) -> Result<TestConnectionResult, String>
{
    let dev_name = deviceName.or(device_name);
    let comp_name = computer_name.or(computerName);
    let tcp_port = common_tcp_port.or(commonTcpPort);

    self_network_test_connection_with_params(
        app,
        TestConnectionParams {
            host,
            device,
            device_name: dev_name,
            ip,
            computer_name: comp_name,
            port,
            common_tcp_port: tcp_port,
            timeout_ms,
        },
    )
    .await
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_resolve_common_tcp_port()
    {
        assert_eq!(
            resolve_common_tcp_port("HTTP"),
            Some(Port::try_from(80).unwrap())
        );
        assert_eq!(
            resolve_common_tcp_port("https"),
            Some(Port::try_from(443).unwrap())
        );
        assert_eq!(
            resolve_common_tcp_port("ssh"),
            Some(Port::try_from(22).unwrap())
        );
        assert_eq!(
            resolve_common_tcp_port("8080"),
            Some(Port::try_from(8080).unwrap())
        );
        assert_eq!(resolve_common_tcp_port("invalid_service"), None);
    }

    #[test]
    fn test_test_connection_result_serialization()
    {
        let result = TestConnectionResult {
            success: true,
            output: "Connection test successful".to_string(),
            computer_name: "localhost".to_string(),
            remote_address: "127.0.0.1".to_string(),
            remote_port: Some(Port::try_from(80).unwrap()),
            interface_alias: None,
            source_address: Some("127.0.0.1".to_string()),
            ping_succeeded: true,
            ping_reply_details: Some("Reply received".to_string()),
            tcp_test_succeeded: Some(true),
            latency_ms: Some(5),
        };
        let serialized = serde_json::to_string(&result).unwrap();
        assert!(serialized.contains("ComputerName") || serialized.contains("computer_name"));
    }

    #[tokio::test]
    async fn test_network_test_connection_core_localhost()
    {
        let res = network_test_connection_core(
            "localhost".to_string(),
            "127.0.0.1".parse().unwrap(),
            None,
            None,
            Some(1000),
        )
        .await;

        assert!(res.is_ok());
        let result = res.unwrap();
        assert_eq!(result.computer_name, "localhost");
        assert_eq!(result.remote_address, "127.0.0.1");
    }
}
