use crate::schema::route::{RouteEntry, RouteMetadata, UniversalRouteTable};
use chrono::Utc;

pub fn parse_windows_route(stdout: &str) -> Result<UniversalRouteTable, String>
{
    let mut entries = Vec::new();
    let mut in_ipv4_routes = false;

    // IPv4 Route Table active routes section starts after "Active Routes:" under "IPv4 Route Table"
    // and ends with "=====================" or "IPv6 Route Table" or "Persistent Routes:"
    for line in stdout.lines()
    {
        let line = line.trim();
        if line.is_empty()
        {
            continue;
        }

        if line.contains("IPv4 Route Table") || line.contains("IPv4 ルート テーブル")
        {
            in_ipv4_routes = true;
            continue;
        }

        if in_ipv4_routes
            && (line.contains("IPv6 Route Table")
                || line.contains("IPv6 ルート テーブル")
                || line.contains("Persistent Routes:")
                || line.contains("固定ルート:"))
        {
            in_ipv4_routes = false;
            continue;
        }

        if !in_ipv4_routes
        {
            continue;
        }

        // Skip headers
        if line.contains("Network Destination")
            || line.contains("ネットワーク宛先")
            || line.contains("Active Routes:")
            || line.contains("アクティブ ルート:")
            || line.starts_with("====")
        {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5
        {
            let dest = parts[0].to_string();
            let netmask = parts[1].to_string();
            let gateway = parts[2].to_string();
            let interface = parts[3].to_string();
            let metric_str = parts[4];

            // Validate if dest is a valid IP or 0.0.0.0
            if dest.parse::<std::net::IpAddr>().is_ok() || dest == "0.0.0.0"
            {
                let metric = metric_str.parse::<i32>().ok();
                let destination = if netmask == "255.255.255.255"
                {
                    dest
                }
                else if dest == "0.0.0.0" && netmask == "0.0.0.0"
                {
                    "default".to_string()
                }
                else
                {
                    format!("{}/{}", dest, netmask)
                };

                entries.push(RouteEntry {
                    destination,
                    gateway,
                    flags: None,
                    interface,
                    metric,
                });
            }
        }
    }

    Ok(UniversalRouteTable {
        version: "1.0".to_string(),
        metadata: RouteMetadata {
            generated_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            source_device: "localhost".to_string(),
            os_type: "windows".to_string(),
        },
        routes: entries,
    })
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_parse_windows_route_success()
    {
        let sample_output = r#"
===========================================================================
IPv4 Route Table
===========================================================================
Active Routes:
Network Destination        Netmask          Gateway       Interface  Metric
          0.0.0.0          0.0.0.0      192.168.1.1    192.168.1.100     25
        127.0.0.0        255.0.0.0         On-link         127.0.0.1    306
        127.0.0.1  255.255.255.255         On-link         127.0.0.1    306
===========================================================================
Persistent Routes:
  None
"#;
        let parsed = parse_windows_route(sample_output).unwrap();
        assert_eq!(parsed.routes.len(), 3);

        assert_eq!(parsed.routes[0].destination, "default");
        assert_eq!(parsed.routes[0].gateway, "192.168.1.1");
        assert_eq!(parsed.routes[0].interface, "192.168.1.100");
        assert_eq!(parsed.routes[0].metric, Some(25));

        assert_eq!(parsed.routes[2].destination, "127.0.0.1");
        assert_eq!(parsed.routes[2].gateway, "On-link");
        assert_eq!(parsed.routes[2].interface, "127.0.0.1");
        assert_eq!(parsed.routes[2].metric, Some(306));
    }
}
