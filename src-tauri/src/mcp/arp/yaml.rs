use crate::snapshot::SnapshotManager;

pub fn quote_yaml_strings(yaml: &str) -> String {
    let mut result = String::new();
    for line in yaml.lines() {
        if let Some(colon_idx) = line.find(':') {
            let key_part = &line[..colon_idx];
            let val_part = line[colon_idx + 1..].trim();

            let trimmed_key = key_part.trim().trim_start_matches('-').trim();

            if [
                "version",
                "generated_at",
                "source_device",
                "os_type",
                "ip_address",
                "mac_address",
                "type",
                "interface",
            ]
            .contains(&trimmed_key)
                && !val_part.is_empty()
            {
                let mut clean_val = val_part;
                if (clean_val.starts_with('"') && clean_val.ends_with('"'))
                    || (clean_val.starts_with('\'') && clean_val.ends_with('\''))
                {
                    clean_val = &clean_val[1..clean_val.len() - 1];
                }

                let indent_len = key_part.len() - key_part.trim_start().len();
                let indent = &key_part[..indent_len];
                let key_name = key_part.trim();

                result.push_str(&format!("{}{}: \"{}\"\n", indent, key_name, clean_val));
                continue;
            }
        }
        result.push_str(line);
        result.push('\n');
    }
    result
}

pub fn save_validated_yaml(
    app: &tauri::AppHandle,
    device_name: &str,
    yaml_content: &str,
) -> Result<std::path::PathBuf, String> {
    let quoted_yaml = quote_yaml_strings(yaml_content);
    let mut manager = SnapshotManager::new(app)
        .map_err(|e| format!("Failed to create SnapshotManager: {}", e))?;
    match manager.save_artifact(device_name, "arp.yaml", &quoted_yaml) {
        Ok(path) => {
            let _ = manager.update_current_link(path.parent().unwrap());
            Ok(path)
        }
        Err(e) => Err(format!("Failed to save YAML artifact: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quote_yaml_strings() {
        let input = r#"version: 1.0
metadata:
  generated_at: 2026-06-13T13:51:38Z
  source_device: Core-Router-01
  os_type: routeros
arp_table:
  - ip_address: 192.168.1.1
    mac_address: 00:11:22:33:44:55
    type: dynamic
    interface: Ethernet1
    age_seconds: 120"#;

        let expected = r#"version: "1.0"
metadata:
  generated_at: "2026-06-13T13:51:38Z"
  source_device: "Core-Router-01"
  os_type: "routeros"
arp_table:
  - ip_address: "192.168.1.1"
    mac_address: "00:11:22:33:44:55"
    type: "dynamic"
    interface: "Ethernet1"
    age_seconds: 120
"#;
        assert_eq!(quote_yaml_strings(input), expected);
    }
}
