use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};
use std::net::{IpAddr, ToSocketAddrs};
use tokio::time::Duration;

pub async fn run_baseline(ip: IpAddr) {
    let payload = vec![0u8; 32];
    for ttl in 1..=5 {
        let config = match ip {
            IpAddr::V4(_) => Config::builder().kind(ICMP::V4).ttl(ttl).build(),
            IpAddr::V6(_) => Config::builder().kind(ICMP::V6).ttl(ttl).build(),
        };

        let client = Client::new(&config).unwrap();
        let mut pinger = client.pinger(ip, PingIdentifier(ttl as u16)).await;
        pinger.timeout(Duration::from_millis(10));

        let _ = pinger.ping(PingSequence(ttl as u16), &payload).await;
    }
}

pub async fn run_optimized(ip: IpAddr) {
    let payload = vec![0u8; 32];

    // We only create one client with default config/ttl,
    // wait, how to set TTL per ping instead of config?
    // Surge ping might not support it per ping?
}
