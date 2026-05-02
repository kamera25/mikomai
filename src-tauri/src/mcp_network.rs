use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};
use std::net::{IpAddr, ToSocketAddrs};
use tokio::time::Duration;
use serde::{Deserialize, Serialize};

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
pub async fn network_ping(host: String) -> Result<PingResult, String> {
    let ip: IpAddr = match host.parse() {
        Ok(ip) => ip,
        Err(_) => tokio::task::spawn_blocking(move || resolve_host(&host))
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

    let mut output = format!("Pinging {} with 32 bytes of data:\n", ip);
    let mut success_count = 0;

    for seq in 0..4 {
        let payload = vec![0u8; 32];
        match pinger.ping(PingSequence(seq as u16), &payload).await {
            Ok((_, duration)) => {
                output.push_str(&format!("Reply from {}: time={:?}\n", ip, duration));
                success_count += 1;
            }
            Err(e) => {
                output.push_str(&format!("Request timed out. ({})\n", e));
            }
        }
    }

    Ok(PingResult {
        success: success_count > 0,
        output,
    })
}

#[tauri::command]
pub async fn network_traceroute(host: String) -> Result<TracerouteResult, String> {
    let ip: IpAddr = match host.parse() {
        Ok(ip) => ip,
        Err(_) => tokio::task::spawn_blocking(move || resolve_host(&host))
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
