use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};
use std::net::{IpAddr, ToSocketAddrs};
use tokio::time::Duration;
use serde::{Deserialize, Serialize};
use crate::connections::resolve_host_with_mcp;

#[derive(Serialize, Deserialize, Debug)]
pub struct PingResult {
    pub success: bool,
    pub output: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TracerouteResult {
    pub success: bool,
    pub output: String,
}

fn resolve_host(host: &str) -> Result<IpAddr, String> {
    let addrs = format!("{}:80", host).to_socket_addrs().map_err(|e| e.to_string())?;
    addrs.into_iter().next().map(|a| a.ip()).ok_or("Could not resolve host".to_string())
}

#[tauri::command]
pub async fn network_ping(
    app: tauri::AppHandle,
    host: String,
    size: Option<usize>,
    count: Option<u32>,
    df: Option<bool>,
) -> Result<PingResult, String> {
    let resolved_host = resolve_host_with_mcp(&app, &host);
    let df_val = df.unwrap_or(false);
    
    // If DF is requested, use system ping fallback (macOS/Linux)
    if df_val {
        return run_system_ping(&resolved_host, size, count, true).await;
    }

    let ip: IpAddr = match resolved_host.parse() {
        Ok(ip) => ip,
        Err(_) => tokio::task::spawn_blocking(move || resolve_host(&resolved_host))
            .await
            .map_err(|e| e.to_string())??,
    };

    let config = match ip {
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

    for seq in 0..ping_count {
        let payload = vec![0u8; payload_size];
        match pinger.ping(PingSequence(seq as u16), &payload).await {
            Ok((_, duration)) => {
                output.push_str(&format!("Reply from {}: time={:?}\n", ip, duration));
                success_count += 1;
            }
            Err(e) => {
                output.push_str(&format!("Request timed out. ({})\n", e));
            }
        }
        if seq < ping_count - 1 {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    output.push_str(&format!("\n--- {} ping statistics ---\n", ip));
    output.push_str(&format!("{} packets transmitted, {} received, {}% packet loss\n", 
        ping_count, success_count, (ping_count - success_count) * 100 / ping_count));

    Ok(PingResult {
        success: success_count > 0,
        output,
    })
}

async fn run_system_ping(host: &str, size: Option<usize>, count: Option<u32>, df: bool) -> Result<PingResult, String> {
    use std::process::Command;
    
    let mut cmd = Command::new("ping");
    
    // Mac and Linux differ slightly in arguments
    // On Mac: -s size -c count -D (for DF)
    // On Linux: -s size -c count -M do (for DF)
    
    if let Some(s) = size {
        cmd.arg("-s").arg(s.to_string());
    }
    
    if let Some(c) = count {
        cmd.arg("-c").arg(c.to_string());
    } else {
        cmd.arg("-c").arg("4");
    }
    
    if df {
        #[cfg(target_os = "macos")]
        cmd.arg("-D");
        #[cfg(target_os = "linux")]
        cmd.arg("-M").arg("do");
    }
    
    cmd.arg(host);
    
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("ping {} {} {} {}", 
            if let Some(s) = size { format!("-s {}", s) } else { "".to_string() },
            if let Some(c) = count { format!("-c {}", c) } else { "-c 4".to_string() },
            if df { 
                #[cfg(target_os = "macos")] { "-D" }
                #[cfg(target_os = "linux")] { "-M do" }
                #[cfg(not(any(target_os = "macos", target_os = "linux")))] { "" }
            } else { "" },
            host
        ))
        .output()
        .map_err(|e| format!("Failed to execute system ping: {}", e))?;
    
    Ok(PingResult {
        success: output.status.success(),
        output: String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

#[tauri::command]
pub async fn network_traceroute(app: tauri::AppHandle, host: String) -> Result<TracerouteResult, String> {
    let resolved_host = resolve_host_with_mcp(&app, &host);
    let ip: IpAddr = match resolved_host.parse() {
        Ok(ip) => ip,
        Err(_) => tokio::task::spawn_blocking(move || resolve_host(&resolved_host))
            .await
            .map_err(|e| e.to_string())??,
    };

    // FxPing manually builds a traceroute by iterating TTLs with surge-ping
    // because `traceroute` crate might have limitations or missing exports.
    // The previous implementation that iterates TTLs matches FxPing exactly!

    let mut output = format!("Tracing route to {} over a maximum of 30 hops:\n\n", ip);
    let mut success = false;
    let payload = vec![0u8; 32];

    for ttl in 1..=30 {
        let config = match ip {
            IpAddr::V4(_) => Config::builder().kind(ICMP::V4).ttl(ttl).build(),
            IpAddr::V6(_) => Config::builder().kind(ICMP::V6).ttl(ttl).build(),
        };

        let client = Client::new(&config).map_err(|e| e.to_string())?;
        let mut pinger = client.pinger(ip, PingIdentifier(ttl as u16)).await;
        pinger.timeout(Duration::from_secs(2));

        match pinger.ping(PingSequence(ttl as u16), &payload).await {
            Ok((packet, duration)) => {
                let hop_ip: IpAddr = match packet {
                    surge_ping::IcmpPacket::V4(p) => p.get_real_dest().into(),
                    surge_ping::IcmpPacket::V6(p) => p.get_real_dest().into(),
                };
                println!("Hop {}: {:?} from {}", ttl, duration, hop_ip);
                output.push_str(&format!("{:2}  {:?}  {}\n", ttl, duration, hop_ip));

                if hop_ip == ip {
                    output.push_str("\nTrace complete.\n");
                    success = true;
                    break;
                }
            }
            Err(e) => {
                println!("Hop {}: Timeout or Error: {:?}", ttl, e);
                output.push_str(&format!("{:2}  *        Request timed out.\n", ttl));
            }
        }
    }

    Ok(TracerouteResult {
        success,
        output,
    })
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HostListResult {
    pub success: bool,
    pub output: String,
}

#[tauri::command]
pub async fn network_get_hosts(app: tauri::AppHandle) -> Result<HostListResult, String> {
    use crate::connections::{load_connections, get_mcp_hosts};
    
    let var_name = "登録されている接続可能なホスト一覧:\n\n".to_string();
    let mut output = var_name;
    output.push_str("| ホスト名 | IPアドレス | 接続タイプ | ソース |\n");
    output.push_str("|----------|------------|------------|--------|\n");

    let mut count = 0;

    // Load local connections
    if let Ok(connections) = load_connections(app.clone()) {
        for conn in connections {
            output.push_str(&format!("| {} | {} | {} | ローカル設定 |\n", conn.hostname, conn.ip, conn.conn_type));
            count += 1;
        }
    }

    // Load MCP hosts
    if let Ok(mcp_hosts) = get_mcp_hosts() {
        for host in mcp_hosts {
            output.push_str(&format!("| {} | {} | {} | MCPレジストリ |\n", host.hostname, host.ip, host.device_type));
            count += 1;
        }
    }

    if count == 0 {
        output = "登録されているホストが見つかりませんでした。".to_string();
    }

    Ok(HostListResult {
        success: true,
        output,
    })
}

