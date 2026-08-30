use crate::connections::IpAddress;
use crate::mcp::protocol::McpToolResult;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};
use tokio::time::Duration;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TracerouteParams {
    pub host: Option<String>,
    pub device: Option<String>,
    pub ip: Option<IpAddress>,
}

pub type TracerouteResult = McpToolResult;

#[cfg(test)]
fn resolve_host(host: &str) -> Result<IpAddr, String> {
    use std::net::ToSocketAddrs;
    let addrs = format!("{}:80", host)
        .to_socket_addrs()
        .map_err(|e| e.to_string())?;
    addrs
        .into_iter()
        .next()
        .map(|a| a.ip())
        .ok_or("Could not resolve host".to_string())
}

pub async fn self_network_traceroute_with_params(
    app: tauri::AppHandle,
    params: TracerouteParams,
) -> Result<TracerouteResult, String> {
    let host_args = crate::mcp::args::HostArgs {
        host: params.host,
        device: params.device,
        device_name: None,
        ip: params.ip,
    };
    let (_target_host, ip_addr) = crate::mcp::args::resolve_host_args(&app, &host_args).await?;

    let mut output = format!(
        "Tracing route to {} over a maximum of 30 hops:\n\n",
        ip_addr
    );
    let mut success = false;
    let payload = vec![0u8; 32];

    for ttl in 1..=30 {
        let config = match ip_addr {
            IpAddr::V4(_) => Config::builder().kind(ICMP::V4).ttl(ttl).build(),
            IpAddr::V6(_) => Config::builder().kind(ICMP::V6).ttl(ttl).build(),
        };

        let client = Client::new(&config).map_err(|e| e.to_string())?;
        let mut pinger = client.pinger(ip_addr, PingIdentifier(ttl as u16)).await;
        pinger.timeout(Duration::from_secs(2));

        match pinger.ping(PingSequence(ttl as u16), &payload).await {
            Ok((packet, duration)) => {
                let hop_ip: IpAddr = match packet {
                    surge_ping::IcmpPacket::V4(p) => p.get_real_dest().into(),
                    surge_ping::IcmpPacket::V6(p) => p.get_real_dest().into(),
                };
                output.push_str(&format!("{:2}  {:?}  {}\n", ttl, duration, hop_ip));

                if hop_ip == ip_addr {
                    output.push_str("\nTrace complete.\n");
                    success = true;
                    break;
                }
            }
            Err(_) => {
                output.push_str(&format!("{:2}  *        Request timed out.\n", ttl));
            }
        }
    }

    Ok(TracerouteResult { success, output })
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn self_network_traceroute(
    app: tauri::AppHandle,
    host: Option<String>,
    device: Option<String>,
    deviceName: Option<String>,
    device_name: Option<String>,
    ip: Option<IpAddress>,
) -> Result<TracerouteResult, String> {
    let target_device = device.or(deviceName).or(device_name);
    self_network_traceroute_with_params(
        app,
        TracerouteParams {
            host,
            device: target_device,
            ip,
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_traceroute_result_serialization() {
        let result = TracerouteResult {
            success: true,
            output: "Trace complete.".to_string(),
        };
        let serialized = serde_json::to_string(&result).unwrap();
        assert_eq!(serialized, r#"{"success":true,"output":"Trace complete."}"#);
    }

    #[test]
    fn test_resolve_host_ip() {
        let ip = resolve_host("127.0.0.1");
        assert!(ip.is_ok());
        assert_eq!(ip.unwrap().to_string(), "127.0.0.1");
    }
}
