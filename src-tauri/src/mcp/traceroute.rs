use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};
use std::net::{IpAddr, ToSocketAddrs};
use tokio::time::Duration;
use serde::{Deserialize, Serialize};
use crate::connections::resolve_host_with_mcp;

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
pub async fn network_traceroute(app: tauri::AppHandle, host: String) -> Result<TracerouteResult, String> {
    let resolved_host = resolve_host_with_mcp(&app, &host);
    let ip: IpAddr = match resolved_host.parse() {
        Ok(ip) => ip,
        Err(_) => tokio::task::spawn_blocking(move || resolve_host(&resolved_host))
            .await
            .map_err(|e| e.to_string())??,
    };

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
                output.push_str(&format!("{:2}  {:?}  {}\n", ttl, duration, hop_ip));

                if hop_ip == ip {
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

    Ok(TracerouteResult {
        success,
        output,
    })
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
}
