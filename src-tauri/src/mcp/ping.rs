use crate::connections::resolve_host_with_mcp;
use crate::mcp::protocol::McpToolResult;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, ToSocketAddrs};
use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};
use tokio::time::Duration;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PingParams
{
    pub host: Option<String>,
    pub device: Option<String>,
    pub device_name: Option<String>,
    pub ip: Option<String>,
    pub size: Option<usize>,
    pub count: Option<u32>,
    pub df: Option<bool>,
}

pub type PingResult = McpToolResult;

fn resolve_host(host: &str) -> Result<IpAddr, String>
{
    let addrs = format!("{}:80", host)
        .to_socket_addrs()
        .map_err(|e| e.to_string())?;
    addrs
        .into_iter()
        .next()
        .map(|a| a.ip())
        .ok_or("Could not resolve host".to_string())
}

pub async fn network_ping_core(
    resolved_host: String,
    size: Option<usize>,
    count: Option<u32>,
    df: Option<bool>,
) -> Result<PingResult, String>
{
    let df_val = df.unwrap_or(false);

    // If DF is requested, use system ping fallback (macOS/Linux)
    if df_val
    {
        return run_system_ping(&resolved_host, size, count, true).await;
    }

    let ip: IpAddr = match resolved_host.parse()
    {
        Ok(ip) => ip,
        Err(_) => tokio::task::spawn_blocking(move || resolve_host(&resolved_host))
            .await
            .map_err(|e| e.to_string())??,
    };

    let config = match ip
    {
        IpAddr::V4(_) => Config::builder().kind(ICMP::V4).build(),
        IpAddr::V6(_) => Config::builder().kind(ICMP::V6).build(),
    };

    let client = Client::new(&config).map_err(|e| e.to_string())?;
    let mut pinger = client.pinger(ip, PingIdentifier(0)).await;
    pinger.timeout(Duration::from_secs(1));

    let payload_size = size.unwrap_or(32);
    let ping_count = count.unwrap_or(4);

    let mut output = format!("Pinging {} with {} bytes of data:\n", ip, payload_size);
    let mut success_count = 0;

    for seq in 0..ping_count
    {
        let payload = vec![0u8; payload_size];
        match pinger.ping(PingSequence(seq as u16), &payload).await
        {
            Ok((_, duration)) =>
            {
                output.push_str(&format!("Reply from {}: time={:?}\n", ip, duration));
                success_count += 1;
            }
            Err(e) =>
            {
                output.push_str(&format!("Request timed out. ({})\n", e));
            }
        }
        if seq < ping_count - 1
        {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    output.push_str(&format!("\n--- {} ping statistics ---\n", ip));
    output.push_str(&format!(
        "{} packets transmitted, {} received, {}% packet loss\n",
        ping_count,
        success_count,
        (ping_count - success_count) * 100 / ping_count
    ));

    Ok(PingResult {
        success: success_count > 0,
        output,
    })
}

pub async fn self_network_ping_with_params(
    app: tauri::AppHandle,
    params: PingParams,
) -> Result<PingResult, String>
{
    let target_host = crate::mcp::args::normalize_host_args(
        &app,
        params.host,
        params.device,
        params.device_name.clone(),
        params.device_name,
        params.ip,
    )?;
    let resolved_host = resolve_host_with_mcp(&app, &target_host);
    let app_clone = app.clone();
    let resolved_host_clone = resolved_host.clone();
    let ip_addr = tokio::task::spawn_blocking(move || {
        crate::connections::resolve_host_with_preference(&app_clone, &resolved_host_clone)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    network_ping_core(ip_addr.to_string(), params.size, params.count, params.df).await
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn self_network_ping(
    app: tauri::AppHandle,
    host: Option<String>,
    device: Option<String>,
    deviceName: Option<String>,
    device_name: Option<String>,
    ip: Option<String>,
    size: Option<usize>,
    count: Option<u32>,
    df: Option<bool>,
) -> Result<PingResult, String>
{
    let dev_name = deviceName.or(device_name);
    self_network_ping_with_params(
        app,
        PingParams {
            host,
            device,
            device_name: dev_name,
            ip,
            size,
            count,
            df,
        },
    )
    .await
}

async fn run_system_ping(
    host: &str,
    size: Option<usize>,
    count: Option<u32>,
    df: bool,
) -> Result<PingResult, String>
{
    use crate::mcp::safe_cmd::resolve_safe_command_path;
    use std::process::Command;

    let is_ipv6 = host.parse::<std::net::Ipv6Addr>().is_ok();
    let ping_cmd = if is_ipv6 { "ping6" } else { "ping" };
    let ping_path = resolve_safe_command_path(ping_cmd)?;

    let mut cmd = Command::new(&ping_path);

    if let Some(s) = size
    {
        cmd.arg("-s").arg(s.to_string());
    }

    if let Some(c) = count
    {
        cmd.arg("-c").arg(c.to_string());
    }
    else
    {
        cmd.arg("-c").arg("4");
    }

    if df
    {
        #[cfg(target_os = "macos")]
        {
            if !is_ipv6
            {
                cmd.arg("-D");
            }
        }
        #[cfg(target_os = "linux")]
        {
            cmd.arg("-M").arg("do");
        }
    }

    cmd.arg(host);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute system ping ({}): {}", ping_cmd, e))?;

    Ok(PingResult {
        success: output.status.success(),
        output: String::from_utf8_lossy(&output.stdout).to_string()
            + &String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_ping_result_serialization()
    {
        let result = PingResult {
            success: true,
            output: "Ping successful".to_string(),
        };
        let serialized = serde_json::to_string(&result).unwrap();
        assert_eq!(serialized, r#"{"success":true,"output":"Ping successful"}"#);
    }

    #[test]
    fn test_resolve_host_ip()
    {
        let ip = resolve_host("127.0.0.1");
        assert!(ip.is_ok());
        assert_eq!(ip.unwrap().to_string(), "127.0.0.1");
    }

    #[test]
    fn test_resolve_host_domain()
    {
        let ip = resolve_host("localhost");
        assert!(ip.is_ok());
        let ip_str = ip.unwrap().to_string();
        assert!(ip_str == "127.0.0.1" || ip_str == "::1");
    }

    #[tokio::test]
    async fn test_network_ping_core_localhost()
    {
        let result =
            network_ping_core("127.0.0.1".to_string(), Some(32), Some(1), Some(true)).await;
        assert!(result.is_ok());
        let ping_res = result.unwrap();
        assert!(ping_res.success);
        assert!(ping_res.output.contains("127.0.0.1"));
    }

    #[tokio::test]
    async fn test_network_ping_core_invalid_host()
    {
        let result = network_ping_core(
            "invalid.localdomain.test".to_string(),
            Some(32),
            Some(1),
            Some(false),
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_system_ping_localhost()
    {
        let result = run_system_ping("127.0.0.1", Some(56), Some(1), false).await;
        assert!(result.is_ok());
        let ping_res = result.unwrap();
        assert!(ping_res.success);
    }
}
