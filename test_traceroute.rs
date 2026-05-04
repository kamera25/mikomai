use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};
use std::net::{IpAddr, ToSocketAddrs};
use tokio::time::Duration;

#[tokio::main]
async fn main() {
    let host = "127.0.0.1";
    let ip: IpAddr = host.parse().unwrap();
    let payload = vec![0u8; 32];

    // Baseline implementation
    let start = std::time::Instant::now();
    for ttl in 1..=10 {
        let config = match ip {
            IpAddr::V4(_) => Config::builder().kind(ICMP::V4).ttl(ttl).build(),
            IpAddr::V6(_) => Config::builder().kind(ICMP::V6).ttl(ttl).build(),
        };

        let client = Client::new(&config).unwrap();
        let mut pinger = client.pinger(ip, PingIdentifier(ttl as u16)).await;
        pinger.timeout(Duration::from_millis(100));

        let _ = pinger.ping(PingSequence(ttl as u16), &payload).await;
    }
    let baseline_duration = start.elapsed();

    // Optimized implementation
    let start = std::time::Instant::now();
    let config = match ip {
        IpAddr::V4(_) => Config::builder().kind(ICMP::V4).build(),
        IpAddr::V6(_) => Config::builder().kind(ICMP::V6).build(),
    };
    let client = Client::new(&config).unwrap();

    for ttl in 1..=10 {
        let mut pinger = client.pinger(ip, PingIdentifier(ttl as u16)).await;
        // Surge ping doesn't easily expose TTL modification after client creation directly without modifying config
        // Wait, Client has pinger. But the Config holds the TTL.
        // Let's check how surge_ping configures TTL.
    }
    let optimized_duration = start.elapsed();

    println!("Baseline: {:?}", baseline_duration);
    println!("Optimized: {:?}", optimized_duration);
}
