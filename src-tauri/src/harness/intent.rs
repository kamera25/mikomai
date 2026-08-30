//! Deterministic intent classification shared by the dispatcher and harness.
//!
//! Keeping these rules in one place prevents the UI router and the agent
//! runtime from disagreeing about whether a request may change a device.

pub fn is_configuration_change_request(message: &str) -> bool {
    let normalized = message.to_lowercase();
    [
        "設定する",
        "設定して",
        "設定を変更",
        "変更する",
        "追加する",
        "削除する",
        "投入する",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::is_configuration_change_request;

    #[test]
    fn distinguishes_read_only_and_change_requests() {
        assert!(is_configuration_change_request(
            "R1 に hostname edge を設定する"
        ));
        assert!(!is_configuration_change_request(
            "R1 の hostname を確認する"
        ));
    }
}
