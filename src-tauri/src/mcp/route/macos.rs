use crate::schema::route::{RouteEntry, RouteMetadata, UniversalRouteTable};
use chrono::Utc;

pub fn parse_macos_route(stdout: &str) -> Result<UniversalRouteTable, String>
{
    let mut entries = Vec::new();
    let mut in_routing_table = false;

    for line in stdout.lines()
    {
        let line = line.trim();
        if line.is_empty()
        {
            continue;
        }

        if line.starts_with("Destination")
        {
            in_routing_table = true;
            continue;
        }

        if line.starts_with("Routing tables")
            || line.starts_with("Internet:")
            || line.starts_with("Internet6:")
        {
            continue;
        }

        if !in_routing_table
        {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        // BSD netstat -rn output has at least 4 fields for valid routes: Destination, Gateway, Flags, Netif
        if parts.len() >= 4
        {
            let destination = parts[0].to_string();
            let gateway = parts[1].to_string();
            let flags = parts[2].to_string();
            let interface = parts[3].to_string();

            entries.push(RouteEntry {
                destination,
                gateway,
                flags: Some(flags),
                interface,
                metric: None,
            });
        }
    }

    Ok(UniversalRouteTable {
        version: "1.0".to_string(),
        metadata: RouteMetadata {
            generated_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            source_device: "localhost".to_string(),
            os_type: "macos".to_string(),
        },
        routes: entries,
    })
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_parse_macos_route_success()
    {
        let sample_output = r#"
Routing tables

Internet:
Destination        Gateway            Flags               Netif Expire
default            192.168.50.1       UGScg                 en0       
127                127.0.0.1          UCS                   lo0       
127.0.0.1          127.0.0.1          UH                    lo0       
169.254            link#11            UCS                   en0      !

Internet6:
Destination                             Gateway                                 Flags               Netif Expire
default                                 fe80::ae44:f2ff:fe91:faf8%en0           UGcg                  en0       
"#;
        let parsed = parse_macos_route(sample_output).unwrap();
        assert_eq!(parsed.routes.len(), 5);

        assert_eq!(parsed.routes[0].destination, "default");
        assert_eq!(parsed.routes[0].gateway, "192.168.50.1");
        assert_eq!(parsed.routes[0].flags, Some("UGScg".to_string()));
        assert_eq!(parsed.routes[0].interface, "en0");

        assert_eq!(parsed.routes[4].destination, "default");
        assert_eq!(parsed.routes[4].gateway, "fe80::ae44:f2ff:fe91:faf8%en0");
        assert_eq!(parsed.routes[4].flags, Some("UGcg".to_string()));
        assert_eq!(parsed.routes[4].interface, "en0");
    }
}
