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

fn is_explanatory_request(normalized: &str) -> bool {
    [
        "とは", "仕組み", "解説", "設定例", "サンプル", "作成して", "生成して", "変換して",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

fn is_configuration_change_request(normalized: &str) -> bool {
    [
        "設定する",
        "設定して",
        "設定を変更",
        "変更する",
        "追加する",
        "削除する",
        "投入する",
        "hostname",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

/// Returns the least-powerful execution model capable of handling `message`.
///
/// This is deliberately deterministic. Live investigations always use
/// `AgentLoop`; there is no single-turn investigation worker to fall back to.
/// Explanatory and authoring requests remain workers unless they explicitly
/// request an observation of the current environment.
pub fn select_dispatch_mode(message: &str) -> DispatchMode {
    let normalized = message.to_lowercase();

    // Configuration changes always enter AgentLoop. The Agent uses the
    // deterministic RAG-template builder, never the standalone Builder LLM.
    if is_configuration_change_request(&normalized) {
        return DispatchMode::Agent;
    }

    let agent_markers = [
        // Explicit delegation
        "自動で調査",
        "自律調査",
        "調査して",
        "調べて",
        "確認して",
        "取得して",
        "表示して",
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
        "状態を確認",
        "設定を確認",
        "設定確認",
        "構成を確認",
        "ログを確認",
        "情報を取得",
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

    let explicitly_live = agent_markers
        .iter()
        .any(|marker| normalized.contains(marker));
    let is_explanation = is_explanatory_request(&normalized);

    if explicitly_live && !is_explanation {
        DispatchMode::Agent
    } else {
        DispatchMode::Worker
    }
}

/// Upgrades requests that name a registered device to `AgentLoop` when they
/// are asking for that device's current state. This matches the router's
/// investigation intent without ever instantiating an investigation worker.
pub fn select_dispatch_mode_for_request(
    app: &tauri::AppHandle,
    message: &str,
) -> DispatchMode {
    let mode = select_dispatch_mode(message);
    if mode == DispatchMode::Agent || is_explanatory_request(&message.to_lowercase()) {
        return mode;
    }

    if !crate::llm::worker::resolve_device_contexts(app, message).is_empty() {
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
        assert_eq!(
            select_dispatch_mode("R1 の状態を調べて"),
            DispatchMode::Agent
        );
        assert_eq!(
            select_dispatch_mode("NakaokuGW の設定を確認して"),
            DispatchMode::Agent
        );
    }

    #[test]
    fn conceptual_requests_do_not_start_an_agent() {
        assert_eq!(
            select_dispatch_mode("OSPF の状態遷移とは？"),
            DispatchMode::Worker
        );
    }

    #[test]
    fn configuration_changes_do_not_route_to_builder_worker() {
        assert_eq!(
            select_dispatch_mode("F220 に hostname aaa を設定する"),
            DispatchMode::Agent
        );
    }
}
