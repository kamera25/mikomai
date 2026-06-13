use crate::schema::arp::{UniversalArpTable, ArpMetadata, ArpEntry, ArpEntryType};
use chrono::Utc;

pub fn parse_windows_arp(stdout: &str) -> Result<UniversalArpTable, String> {
    let mut entries = Vec::new();
    let mut current_interface = "unknown".to_string();

    // Example of Windows `arp -a` output:
    // Interface: 192.168.1.100 --- 0x3
    //   Internet Address      Physical Address      Type
    //   192.168.1.1           ac-44-f2-91-fa-f8     dynamic
    //   224.0.0.22            01-00-5e-00-00-16     static
    
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Check if this line defines an interface
        if line.starts_with("Interface:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 1 {
                current_interface = parts[1].to_string();
            }
            continue;
        }

        // Skip header lines
        if line.contains("Internet Address") || line.contains("インターネット アドレス") {
            continue;
        }

        // Parse entry lines (they should have IP, Physical Address, and Type)
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let ip_address = parts[0].to_string();
            let raw_mac = parts[1].to_string();
            let raw_type = parts[2].to_lowercase();

            // Ignore invalid/loopback IP address entries that are not valid IPv4/IPv6 if necessary,
            // or simply validate IP via std::net::IpAddr
            if ip_address.parse::<std::net::IpAddr>().is_err() {
                continue;
            }

            // Normalization of MAC Address: Windows uses hyphens (e.g. ac-44-f2-91-fa-f8). We need to convert it to colons.
            let mac_address = if raw_mac == "---" || raw_mac.to_lowercase() == "invalid" {
                "00:00:00:00:00:00".to_string()
            } else {
                let colons_mac = raw_mac.replace('-', ":").to_lowercase();
                // Ensure proper padding (e.g., if any octet is single digit, which is rare on Windows but just in case)
                let parts: Vec<&str> = colons_mac.split(':').collect();
                if parts.len() == 6 {
                    let mut padded_parts = Vec::new();
                    for part in parts {
                        if part.len() == 1 {
                            padded_parts.push(format!("0{}", part));
                        } else {
                            padded_parts.push(part.to_string());
                        }
                    }
                    padded_parts.join(":")
                } else {
                    colons_mac
                }
            };

            let entry_type = match raw_type.as_str() {
                "static" | "静的" => ArpEntryType::Static,
                "invalid" | "無効" => ArpEntryType::Incomplete,
                _ => ArpEntryType::Dynamic,
            };

            entries.push(ArpEntry {
                ip_address,
                mac_address,
                r#type: entry_type,
                interface: current_interface.clone(),
                age_seconds: None,
            });
        }
    }

    Ok(UniversalArpTable {
        version: "1.0".to_string(),
        metadata: ArpMetadata {
            generated_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            source_device: "localhost".to_string(),
            os_type: "windows".to_string(),
        },
        arp_table: entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_windows_arp_success() {
        let sample_output = r#"
Interface: 192.168.50.15 --- 0x12
  Internet Address      Physical Address      Type
  192.168.50.1          ac-44-f2-91-fa-f8     dynamic
  192.168.50.22         01-00-5e-00-00-16     static
"#;
        let parsed = parse_windows_arp(sample_output).unwrap();
        assert_eq!(parsed.arp_table.len(), 2);
        
        assert_eq!(parsed.arp_table[0].ip_address, "192.168.50.1");
        assert_eq!(parsed.arp_table[0].mac_address, "ac:44:f2:91:fa:f8");
        assert_eq!(parsed.arp_table[0].r#type, ArpEntryType::Dynamic);
        assert_eq!(parsed.arp_table[0].interface, "192.168.50.15");

        assert_eq!(parsed.arp_table[1].ip_address, "192.168.50.22");
        assert_eq!(parsed.arp_table[1].mac_address, "01:00:5e:00:00:16");
        assert_eq!(parsed.arp_table[1].r#type, ArpEntryType::Static);
    }
}
