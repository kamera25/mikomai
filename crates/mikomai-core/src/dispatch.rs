//! Deterministic selection of the least-powerful execution model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchMode {
    Worker,
    Agent,
}
fn explanatory(message: &str) -> bool {
    [
        "とは",
        "仕組み",
        "解説",
        "設定例",
        "サンプル",
        "作成して",
        "生成して",
        "変換して",
    ]
    .iter()
    .any(|m| message.contains(m))
}
fn change(message: &str) -> bool {
    [
        "設定する",
        "設定して",
        "変更する",
        "変更して",
        "configure",
        "network_config",
        "vlan を作成",
    ]
    .iter()
    .any(|m| message.contains(m))
}
pub fn select_dispatch_mode(message: &str) -> DispatchMode {
    let normalized = message.to_lowercase();
    if change(&normalized) {
        return DispatchMode::Agent;
    }
    let live = [
        "調査して",
        "調べて",
        "確認して",
        "取得して",
        "診断",
        "疎通",
        "状態確認",
        "障害",
        "show ip",
        "show interface",
        "繋がらない",
        "つながらない",
        "timeout",
        "unreachable",
        "diagnose",
        "troubleshoot",
    ]
    .iter()
    .any(|m| normalized.contains(m));
    if live && !explanatory(&normalized) {
        DispatchMode::Agent
    } else {
        DispatchMode::Worker
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routes_live_requests_to_agent() {
        assert_eq!(
            select_dispatch_mode("R1の状態を確認して"),
            DispatchMode::Agent
        );
        assert_eq!(
            select_dispatch_mode("OSPFの仕組みを解説して"),
            DispatchMode::Worker
        );
        assert_eq!(
            select_dispatch_mode("F220にVLANを設定する"),
            DispatchMode::Agent
        );
    }
}
