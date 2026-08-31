use crate::schema::arp::{ArpEntry, ArpEntryType, ArpMetadata, UniversalArpTable};
use chrono::Utc;

pub fn parse_macos_arp(stdout: &str) -> Result<UniversalArpTable, String> {
    let mut entries = Vec::new();

    // Example: "? (192.168.50.1) at ac:44:f2:91:fa:f8 on en0 ifscope [ethernet]"
    // Example: "? (192.168.50.220) at (incomplete) on en0 ifscope [ethernet]"
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Parse IP Address: starts after '(? (' or similar
        let ip_start = match line.find('(') {
            Some(idx) => idx + 1,
            None => continue,
        };
        let ip_end = match line[ip_start..].find(')') {
            Some(idx) => ip_start + idx,
            None => continue,
        };
        let ip_address = line[ip_start..ip_end].to_string();

        // Parse MAC Address
        let at_idx = match line.find(" at ") {
            Some(idx) => idx + 4,
            None => continue,
        };
        let on_idx = match line[at_idx..].find(" on ") {
            Some(idx) => at_idx + idx,
            None => continue,
        };
        let raw_mac = line[at_idx..on_idx].trim().to_string();

        // Determine Entry Type (check for 'permanent' flag)
        let is_permanent = line.contains(" permanent");
        let is_incomplete = raw_mac == "(incomplete)";

        let entry_type = if is_incomplete {
            ArpEntryType::Incomplete
        } else if is_permanent {
            ArpEntryType::Permanent
        } else {
            ArpEntryType::Dynamic
        };

        // Parse Interface
        let if_start = on_idx + 4;
        let if_end = match line[if_start..].find(' ') {
            Some(idx) => if_start + idx,
            None => line.len(),
        };
        let interface = line[if_start..if_end].trim().to_string();

        // Standardize MAC address to standard colon-separated lowercase with padded octets
        let mac_address = if is_incomplete {
            None
        } else {
            // E.g., e:5a:d9:cf:f3:7c -> 0e:5a:d9:cf:f3:7c
            let parts: Vec<&str> = raw_mac.split(':').collect();
            if parts.len() == 6 {
                let mut padded_parts = Vec::new();
                for part in parts {
                    if part.len() == 1 {
                        padded_parts.push(format!("0{}", part.to_lowercase()));
                    } else {
                        padded_parts.push(part.to_lowercase());
                    }
                }
                Some(padded_parts.join(":"))
            } else {
                Some(raw_mac.to_lowercase())
            }
        };

        entries.push(ArpEntry {
            ip_address,
            mac_address,
            r#type: entry_type,
            interface: Some(interface),
            age_seconds: None,
        });
    }

    Ok(UniversalArpTable {
        version: "1.0".to_string(),
        metadata: ArpMetadata {
            generated_at: Utc::now(),
            source_device: "localhost".to_string(),
            os_type: "macos".to_string(),
        },
        arp_table: entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_macos_arp_success() {
        let sample_output = r#"
? (192.168.50.1) at ac:44:f2:91:fa:f8 on en0 ifscope [ethernet]
? (192.168.50.18) at e:5a:d9:cf:f3:7c on en0 ifscope [ethernet]
? (192.168.50.220) at (incomplete) on en0 ifscope [ethernet]
? (224.0.0.251) at 1:0:5e:0:0:fb on en0 ifscope permanent [ethernet]
"#;
        let parsed = parse_macos_arp(sample_output).unwrap();
        assert_eq!(parsed.arp_table.len(), 4);

        // Check normal entry with standard MAC
        assert_eq!(parsed.arp_table[0].ip_address, "192.168.50.1");
        assert_eq!(parsed.arp_table[0].mac_address.as_deref(), Some("ac:44:f2:91:fa:f8"));
        assert_eq!(parsed.arp_table[0].r#type, ArpEntryType::Dynamic);
        assert_eq!(parsed.arp_table[0].interface.as_deref(), Some("en0"));

        // Check padded MAC
        assert_eq!(parsed.arp_table[1].ip_address, "192.168.50.18");
        assert_eq!(parsed.arp_table[1].mac_address.as_deref(), Some("0e:5a:d9:cf:f3:7c"));

        // Check incomplete
        assert_eq!(parsed.arp_table[2].ip_address, "192.168.50.220");
        assert_eq!(parsed.arp_table[2].mac_address, None);
        assert_eq!(parsed.arp_table[2].r#type, ArpEntryType::Incomplete);

        // Check permanent and padded MAC
        assert_eq!(parsed.arp_table[3].ip_address, "224.0.0.251");
        assert_eq!(parsed.arp_table[3].mac_address.as_deref(), Some("01:00:5e:00:00:fb"));
        assert_eq!(parsed.arp_table[3].r#type, ArpEntryType::Permanent);
    }
}
