//! Redacts credential-bearing network configuration before it enters an LLM
//! prompt, evidence store, or synthesis context.

const SECRET_MARKERS: &[&str] = &[
    "password",
    "secret",
    "community",
    "pre-shared-key",
    "preshared-key",
    "private-key",
    "private key",
    "authentication-key",
    "auth-key",
    "snmp-server user",
    "crypto isakmp key",
    "key-string",
    "tacacs-server key",
    "radius-server key",
    "wpa-psk",
    "wpa psk",
    "ikev2 keyring",
    "pre-shared",
];

pub fn redact_network_secrets(input: &str) -> String {
    let mut secret_block: Option<(usize, bool)> = None;
    input
        .lines()
        .map(|line| {
            let indent = line.chars().take_while(|ch| ch.is_whitespace()).count();
            if let Some((base, redact_only_keys)) = secret_block {
                if !line.trim().is_empty() && indent > base {
                    if !redact_only_keys || line.trim().to_ascii_lowercase().starts_with("key ") {
                        return "[REDACTED: credential-bearing configuration line]";
                    }
                    // Modern IOS `radius server NAME` / `tacacs server NAME`
                    // blocks may contain safe address/port lines before `key`.
                    if normalized_is_server_block(line) {
                        secret_block = Some((indent, true));
                    }
                    return line;
                } else {
                    secret_block = None;
                }
            }
            let normalized = line.to_ascii_lowercase();
            if normalized.starts_with("radius server ") || normalized.starts_with("tacacs server ")
            {
                secret_block = Some((indent, true));
                return line;
            }
            let sensitive = SECRET_MARKERS
                .iter()
                .any(|marker| normalized.contains(marker));
            if sensitive {
                if normalized.contains("keyring")
                    || normalized.contains("private key")
                    || normalized.contains("private-key")
                {
                    secret_block = Some((indent, false));
                }
                "[REDACTED: credential-bearing configuration line]"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalized_is_server_block(line: &str) -> bool {
    let normalized = line.trim().to_ascii_lowercase();
    normalized.starts_with("radius server ") || normalized.starts_with("tacacs server ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_secret_lines_and_preserves_safe_config() {
        let result = redact_network_secrets(
            "hostname edge\nusername admin secret 5 abc123\nsnmp-server community public RO",
        );
        assert!(result.contains("hostname edge"));
        assert_eq!(result.matches("[REDACTED").count(), 2);
        assert!(!result.contains("abc123"));
        assert!(!result.contains("public"));
    }

    #[test]
    fn removes_common_vendor_secret_forms_and_multiline_key_blocks() {
        let result = redact_network_secrets(
            "hostname edge\ntacacs-server key 7 abc\nradius-server key hidden\ncrypto ikev2 keyring VPN\n peer remote\n  pre-shared-key local secret\nset system login user netops authentication encrypted-password $6$hash",
        );
        assert!(result.contains("hostname edge"));
        assert!(!result.contains("abc"));
        assert!(!result.contains("hidden"));
        assert!(!result.contains("secret"));
        assert!(!result.contains("$6$hash"));
        assert!(result.lines().skip(3).all(|line| line.contains("REDACTED")));
    }

    #[test]
    fn redacts_keys_in_modern_aaa_server_blocks_but_keeps_safe_lines() {
        let result = redact_network_secrets(
            "tacacs server TAC1\n address ipv4 192.0.2.1\n key 7 encrypted-tacacs\nradius server RAD1\n address ipv4 192.0.2.2 auth-port 1812\n key 0 clear-radius\nhostname edge",
        );
        assert!(result.contains("address ipv4 192.0.2.1"));
        assert!(result.contains("hostname edge"));
        assert!(!result.contains("encrypted-tacacs"));
        assert!(!result.contains("clear-radius"));
    }
}
