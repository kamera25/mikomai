//! Selects the execution model for an incoming user request.
//!
//! Workers are the default: they are bounded, role-specific LLM calls that
//! answer, draft, summarize, or retrieve knowledge.  The agent loop is only
//! appropriate when the request needs to observe external network state over
//! multiple steps.  Keeping this decision outside prompts prevents an
//! accidental "agent for every chat" architecture.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchMode {
    Worker,
    Agent,
}

/// Returns the least-powerful execution model capable of handling `message`.
///
/// This is deliberately deterministic.  It is a safety gate, not an LLM
/// classification: an LLM may select a specialised worker *after* this gate,
/// but cannot silently escalate an explanatory request into device access.
pub fn select_dispatch_mode(message: &str) -> DispatchMode {
    let normalized = message.to_lowercase();

    let agent_markers = [
        // Explicit delegation
        "自動で調査",
        "自律調査",
        "切り分けて",
        "切り分け",
        "診断して",
        "診断",
        "investigate",
        "diagnose",
        "troubleshoot",
        "autonomously",
        // Requests whose answer depends on live network observations
        "疎通",
        "接続確認",
        "状態確認",
        "障害",
        "ping",
        "traceroute",
        "show ip",
        "show interface",
        "show running",
        "show config",
        "ルーティングを確認",
        "経路を確認",
        "arpを確認",
        "arp を確認",
    ];

    if agent_markers
        .iter()
        .any(|marker| normalized.contains(marker))
    {
        DispatchMode::Agent
    } else {
        DispatchMode::Worker
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explanatory_requests_stay_with_workers() {
        assert_eq!(
            select_dispatch_mode("Yamaha の VLAN 設定を説明して"),
            DispatchMode::Worker
        );
        assert_eq!(
            select_dispatch_mode("Cisco の設定例を作成して"),
            DispatchMode::Worker
        );
    }

    #[test]
    fn live_or_autonomous_requests_use_the_agent() {
        assert_eq!(
            select_dispatch_mode("R1 への疎通を確認して"),
            DispatchMode::Agent
        );
        assert_eq!(
            select_dispatch_mode("障害を自動で調査して"),
            DispatchMode::Agent
        );
        assert_eq!(
            select_dispatch_mode("show ip route を実行して"),
            DispatchMode::Agent
        );
    }
}
