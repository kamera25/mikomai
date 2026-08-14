pub mod ping;
pub mod trace;
pub mod nwdiag;

pub use ping::*;
pub use trace::*;
pub use nwdiag::*;

use regex::Regex;
use serde::Deserialize;
use serde_json::Value;

use super::types::{RouteAction, RoutingDecision, RoutingSource};

#[derive(Debug, Deserialize, Clone)]
pub struct DenyRules {
    pub patterns: Vec<String>,
    pub message: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ShortcutRule {
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub patterns: Vec<String>,
}

fn default_question_keywords() -> Vec<String> {
    vec![
        "とは".to_string(),
        "何".to_string(),
        "？".to_string(),
        "?".to_string(),
        "どう".to_string(),
        "なぜ".to_string(),
        "why".to_string(),
        "what".to_string(),
        "how".to_string(),
    ]
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct FastRouteConfig {
    #[serde(default)]
    pub ping: PingRegexConfig,
    #[serde(default)]
    pub traceroute: ShortcutRule,
    #[serde(default)]
    pub test_connection: ShortcutRule,
    #[serde(default)]
    pub host_list: ShortcutRule,
    #[serde(default)]
    pub arp: ShortcutRule,
    #[serde(default)]
    pub route: ShortcutRule,
    #[serde(default)]
    pub serial_ports: ShortcutRule,
    #[serde(default)]
    pub nwdiag: NwdiagRegexConfig,
    #[serde(default)]
    pub greeting: ShortcutRule,
    #[serde(default = "default_question_keywords")]
    pub question_keywords: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ShortcutRulesConfig {
    pub deny_rules: Option<DenyRules>,
    #[serde(default, alias = "regex_patterns")]
    pub fastroute: FastRouteConfig,
}

impl ShortcutRulesConfig {
    pub fn load() -> Self {
        let yaml_str = include_str!("../config/rules.yaml");
        serde_yaml::from_str(yaml_str).unwrap_or_else(|e| {
            log::error!("Failed to parse rules.yaml: {}", e);
            ShortcutRulesConfig {
                deny_rules: None,
                fastroute: FastRouteConfig::default(),
            }
        })
    }
}

pub(crate) fn has_question_keywords(input: &str, config: &ShortcutRulesConfig) -> bool {
    let lower = input.to_lowercase();
    config
        .fastroute
        .question_keywords
        .iter()
        .any(|kw| lower.contains(&kw.to_lowercase()))
}

pub(crate) fn extract_first_capture(input: &str, patterns: &[String]) -> Option<String> {
    for pattern in patterns {
        if let Ok(re) = Regex::new(pattern) {
            if let Some(caps) = re.captures(input) {
                if let Some(m) = caps.get(1) {
                    return Some(m.as_str().to_string());
                }
            }
        }
    }
    None
}

pub(crate) fn calculate_host_confidence(input: &str, host: &str, config: &ShortcutRulesConfig) -> f64 {
    if has_question_keywords(input, config) {
        0.0
    } else if host.contains('.') || host.contains(':') || host == "localhost" {
        1.0
    } else {
        0.9
    }
}

fn detect_greeting_shortcut(
    input: &str,
    lower_input: &str,
    config: &ShortcutRulesConfig,
) -> Option<(String, Value, String, f64)> {
    let rule = &config.fastroute.greeting;
    if rule.patterns.is_empty() {
        return None;
    }
    let trimmed = input.trim();
    for pattern in &rule.patterns {
        if let Ok(re) = Regex::new(pattern) {
            if re.is_match(trimmed) || re.is_match(lower_input) {
                return Some((
                    rule.action.clone(),
                    serde_json::json!({}),
                    rule.message.trim().to_string(),
                    1.0,
                ));
            }
        }
    }
    None
}

fn detect_simple_shortcut(
    input: &str,
    lower_input: &str,
    rule: &ShortcutRule,
    config: &ShortcutRulesConfig,
) -> Option<(String, Value, String, f64)> {
    if rule.patterns.is_empty() {
        return None;
    }
    for pattern in &rule.patterns {
        if let Ok(re) = Regex::new(pattern) {
            if re.is_match(lower_input) {
                let confidence = if has_question_keywords(input, config) { 0.4 } else { 1.0 };
                return Some((
                    rule.action.clone(),
                    serde_json::json!({}),
                    rule.message.clone(),
                    confidence,
                ));
            }
        }
    }
    None
}

pub fn detect_shortcut_raw(input: &str) -> Option<(String, Value, String, f64)> {
    let config = ShortcutRulesConfig::load();
    let lower_input = input.to_lowercase();
    let reg = &config.fastroute;

    // 1. Greeting
    if let Some(res) = detect_greeting_shortcut(input, &lower_input, &config) {
        return Some(res);
    }

    // 2. Ping
    if let Some(res) = detect_ping_shortcut(input, &config) {
        return Some(res);
    }

    // 3. Traceroute
    if let Some(res) = detect_traceroute_shortcut(input, &lower_input, &config) {
        return Some(res);
    }

    // 4. Simple shortcuts (Test-NetConnection, Host List, ARP, Route, Serial Ports)
    let simple_rules = [
        &reg.test_connection,
        &reg.host_list,
        &reg.arp,
        &reg.route,
        &reg.serial_ports,
    ];
    for rule in simple_rules {
        if let Some(res) = detect_simple_shortcut(input, &lower_input, rule, &config) {
            return Some(res);
        }
    }

    // 5. nwdiag shortcut
    if let Some(res) = detect_nwdiag_shortcut(input, &config) {
        return Some(res);
    }

    None
}

pub fn detect_shortcut(input: &str) -> Option<RoutingDecision> {
    let (tool_name, params, message, confidence) = detect_shortcut_raw(input)?;
    let action = if tool_name == "static_reply" || tool_name.is_empty() {
        RouteAction::StaticReply { message }
    } else {
        RouteAction::DirectToolCall {
            tool_name,
            params,
            message,
        }
    };

    Some(RoutingDecision {
        action,
        confidence,
        device_contexts: Vec::new(),
        source: RoutingSource::Shortcut,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ping_command() {
        let val1 = parse_ping_command("ping 192.168.1.1").unwrap();
        assert_eq!(val1["host"], "192.168.1.1");
        assert!(val1.get("size").is_none());
        assert!(val1.get("count").is_none());

        let val2 = parse_ping_command("8.8.8.8 へピング size 1400 count 5 df").unwrap();
        assert_eq!(val2["host"], "8.8.8.8");
        assert_eq!(val2["size"], 1400);
        assert_eq!(val2["count"], 5);
        assert_eq!(val2["df"], true);
    }

    #[test]
    fn test_detect_shortcut() {
        let config = ShortcutRulesConfig::load();

        // Greeting
        let res = detect_shortcut("こんにちは").unwrap();
        match res.action {
            RouteAction::StaticReply { ref message } => {
                assert!(message.contains("MIKOMAI"));
            }
            _ => panic!("Expected StaticReply"),
        }
        assert_eq!(res.confidence, 1.0);

        let res_intro = detect_shortcut("自己紹介してください").unwrap();
        match res_intro.action {
            RouteAction::StaticReply { ref message } => {
                assert!(message.contains("MIKOMAI"));
            }
            _ => panic!("Expected StaticReply"),
        }
        assert_eq!(res_intro.confidence, 1.0);

        // Ping
        let res = detect_shortcut("ping google.com").unwrap();
        match res.action {
            RouteAction::DirectToolCall { ref tool_name, .. } => {
                assert_eq!(tool_name, "self_network_ping");
            }
            _ => panic!("Expected DirectToolCall"),
        }

        let res_fqdn_no_space = detect_shortcut("dns.googleへPing").unwrap();
        match res_fqdn_no_space.action {
            RouteAction::DirectToolCall { ref tool_name, ref params, .. } => {
                assert_eq!(tool_name, "self_network_ping");
                assert_eq!(params["host"], "dns.google");
            }
            _ => panic!("Expected DirectToolCall"),
        }

        // Ping question fallback (low confidence)
        let res_ping_q = detect_ping_shortcut("ping google.comとは何？", &config).unwrap();
        assert_eq!(res_ping_q.0, "self_network_ping");
        assert!(res_ping_q.3 < 0.8);

        // Traceroute
        let res = detect_shortcut("traceroute 1.1.1.1").unwrap();
        match res.action {
            RouteAction::DirectToolCall { ref tool_name, ref params, .. } => {
                assert_eq!(tool_name, "self_network_traceroute");
                assert_eq!(params["host"], "1.1.1.1");
            }
            _ => panic!("Expected DirectToolCall"),
        }
        assert!(res.confidence >= 0.8);

        // Host List
        let res = detect_shortcut("接続先一覧を確認したい").unwrap();
        match res.action {
            RouteAction::DirectToolCall { ref tool_name, .. } => {
                assert_eq!(tool_name, "network_get_hosts");
            }
            _ => panic!("Expected DirectToolCall"),
        }
        assert!(res.confidence >= 0.8);

        // Local ARP
        let res = detect_shortcut("自機のarpテーブル").unwrap();
        match res.action {
            RouteAction::DirectToolCall { ref tool_name, .. } => {
                assert_eq!(tool_name, "self_network_arp");
            }
            _ => panic!("Expected DirectToolCall"),
        }
        assert!(res.confidence >= 0.8);

        // Local Route
        let res = detect_shortcut("ローカルのルーティングテーブル").unwrap();
        match res.action {
            RouteAction::DirectToolCall { ref tool_name, .. } => {
                assert_eq!(tool_name, "self_network_route");
            }
            _ => panic!("Expected DirectToolCall"),
        }
        assert!(res.confidence >= 0.8);

        // Serial Ports
        let res = detect_shortcut("コンソールポート一覧").unwrap();
        match res.action {
            RouteAction::DirectToolCall { ref tool_name, .. } => {
                assert_eq!(tool_name, "network_list_serial_ports");
            }
            _ => panic!("Expected DirectToolCall"),
        }
        assert!(res.confidence >= 0.8);

        // nwdiag
        let res = detect_shortcut("nwdiagで図を作成して：\nnwdiag {\n  network {\n    web01;\n  }\n}").unwrap();
        match res.action {
            RouteAction::DirectToolCall { ref tool_name, ref params, .. } => {
                assert_eq!(tool_name, "self_network_nwdiag");
                assert_eq!(params["schema"], "nwdiag {\n  network {\n    web01;\n  }\n}");
            }
            _ => panic!("Expected DirectToolCall"),
        }
        assert_eq!(res.confidence, 1.0);

        // None
        assert!(detect_shortcut("普通の質問: NTPって何？").is_none());
    }
}
