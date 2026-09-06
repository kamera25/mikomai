pub fn is_arp_show_command(command: &str) -> bool {
    let normalized = command.trim().to_ascii_lowercase();
    normalized.starts_with("show ") && normalized.split_whitespace().any(|word| word == "arp")
}

pub fn is_route_show_command(command: &str) -> bool {
    let normalized = command.trim().to_ascii_lowercase();
    normalized.starts_with("show ")
        && normalized
            .split_whitespace()
            .any(|word| word == "route" || word == "routing")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_show_commands_only() {
        assert!(is_arp_show_command(" SHOW IP ARP "));
        assert!(is_route_show_command("show ip route"));
        assert!(!is_arp_show_command("configure arp"));
        assert!(!is_route_show_command("display route"));
    }
}
