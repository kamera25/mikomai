//! Deterministic selection of the least-powerful execution model.
//! This policy is shared by desktop runtimes; it has no GUI or device I/O.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchMode {
    Worker,
    FastRouter,
    Agent,
}

/// A full command match is high confidence; partial matches and compound goals
/// stay with the agent. Never infer an execution target from surrounding prose.
pub fn fast_route(message: &str) -> Option<LegacyShortcut> {
    if crate::network::ndp::is_command(message) {
        return Some(LegacyShortcut {
            tool: Some("get_state".into()), target: Some("localhost".into()),
            args: serde_json::json!({"device":"localhost","resource":"ndp"}), reply: None,
        });
    }
    if let Some(shortcut) = explicit_probe_shortcut(message) {
        return Some(shortcut);
    }
    if let Some(shortcut) = port_check_shortcut(message) {
        return Some(shortcut);
    }
    if let Some(shortcut) = local_route_shortcut(message) {
        return Some(shortcut);
    }
    let command = message.trim();
    // Scoped IPv6 hosts are not supported by the legacy extractor.
    if command.contains('%') {
        return None;
    }
    let pattern = regex::Regex::new(
        r"(?ix)^(?:(?:traceroute|trace(?:\s+route)?|トレースルート)\s+[a-z0-9][a-z0-9.:%-]*|(?:ping|ピング|ピン)\s+[a-z0-9][a-z0-9.:%-]*(?:\s+(?:count|回数|回|size|サイズ)\s*\d+)*(?:\s+df)?|(?:tnc\s+|test-netconnection\s+-computername\s+)[a-z0-9][a-z0-9.:%-]*\s+-port\s+\d+)$",
    ).ok()?;
    if !pattern.is_match(command) {
        return None;
    }
    let mut shortcut = legacy_shortcut(command)?;
    let host = shortcut.args["host"].as_str()?;
    // Legacy extraction must consume the complete host token (including IPv6).
    if host.len() > 255 || host.contains('%') {
        return None;
    }
    if shortcut.tool.as_deref() == Some("self_network_test_connection")
        && !shortcut.args["port"].as_u64().is_some_and(|port| (1..=65535).contains(&port))
    {
        return None;
    }
    if shortcut.tool.as_deref() == Some("self_network_ping") {
        let mut seen = std::collections::HashSet::new();
        let tokens = command.split_whitespace().skip(2).collect::<Vec<_>>();
        let mut index = 0;
        while index < tokens.len() {
            let option = tokens[index].to_ascii_lowercase();
            if option == "df" {
                index += 1;
                continue;
            }
            let key = if option.starts_with("size") || option.starts_with("サイズ") { "size" } else { "count" };
            if !seen.insert(key) || shortcut.args[key].as_u64().is_none() {
                return None;
            }
            // The legacy parser supports values joined to Japanese option names.
            index += if option.chars().any(|ch| ch.is_ascii_digit()) { 1 } else { 2 };
        }
        shortcut.args["dont_fragment"] = serde_json::json!(command.split_whitespace().last().is_some_and(|token| token.eq_ignore_ascii_case("df")));
    }
    Some(shortcut)
}
/// Parse command, destination, then options for explicit ping/trace requests.
/// Unknown options are ignored rather than misread as the destination.
fn explicit_probe_shortcut(message: &str) -> Option<LegacyShortcut> {
    let tokens = message.split_whitespace().collect::<Vec<_>>();
    let command = tokens.first()?.to_ascii_lowercase();
    let trace = matches!(command.as_str(), "trace" | "traceroute" | "トレースルート");
    if (!trace && !matches!(command.as_str(), "ping" | "ピング" | "ピン")) || tokens.len() < 2 {
        return None;
    }
    let start = if command == "trace" && tokens.get(1).is_some_and(|s| s.eq_ignore_ascii_case("route")) { 2 } else { 1 };
    // Extract the destination before interpreting options. Numeric option values
    // must never become hosts, and an IP literal takes precedence over DNS text.
    let candidates = tokens.iter().enumerate().skip(start).filter(|(i, token)| {
        let lower = token.to_ascii_lowercase();
        !token.starts_with('-') && token.len() <= 255 && !token.contains('%')
            && !token.chars().all(|ch| ch.is_ascii_digit())
            && token.chars().all(|ch| ch.is_ascii_alphanumeric() || ".-:".contains(ch))
            && !matches!(lower.as_str(), "df" | "count" | "size" | "dont-fragment")
            && !matches!(tokens[i - 1].to_ascii_lowercase().as_str(), "-c" | "count" | "-s" | "size")
    }).collect::<Vec<_>>();
    let ips = candidates.iter().filter(|(_, token)| token.parse::<std::net::IpAddr>().is_ok()).collect::<Vec<_>>();
    let (host_index, host) = if ips.len() == 1 { **ips.first()? } else if ips.is_empty() && candidates.len() == 1 { *candidates.first()? } else { return None; };
    // Keep prose, multiple operations and shell syntax out of direct execution.
    if tokens.iter().skip(start).any(|token| token.chars().any(|ch| ch.is_control() || ";|&$`()<>\\\"'".contains(ch))
        || token.chars().any(|ch| !ch.is_ascii()) && !matches!(*token, "回数" | "回" | "サイズ" | "フラグメント禁止")) { return None; }
    let mut count = None;
    let mut size = None;
    let mut dont_fragment = false;
    let mut index = start;
    while index < tokens.len() {
        if index == host_index { index += 1; continue; }
        let token = tokens[index];
        let option = token.to_ascii_lowercase();
        match option.as_str() {
            "-df" | "-d" | "df" | "dont-fragment" | "フラグメント禁止" => {
                if dont_fragment { return None; }
                dont_fragment = true;
                index += 1;
            }
            "-c" | "count" | "回数" | "回" => {
                if count.is_some() { return None; }
                let value = tokens.get(index + 1)?.parse::<u32>().ok()?;
                if value == 0 || value > 10 { return None; }
                count = Some(value);
                index += 2;
            }
            "-s" | "size" | "サイズ" => {
                if size.is_some() { return None; }
                let value = tokens.get(index + 1)?.parse::<u32>().ok()?;
                if !(1..=65_500).contains(&value) { return None; }
                size = Some(value);
                index += 2;
            }
            _ => {
                // Unknown flags and their values remain options, never destinations.
                index += 1;
            }
        }
    }
    Some(LegacyShortcut {
        tool: Some(if trace { "self_network_traceroute" } else { "self_network_ping" }.into()),
        target: Some("localhost".into()),
        args: serde_json::json!({
            "host":host, "count":count, "size":size, "dont_fragment":dont_fragment,
        }),
        reply: None,
    })
}

/// A single explicit TCP check. Anchoring keeps compound investigations in Agent.
pub fn port_check_shortcut(message: &str) -> Option<LegacyShortcut> {
    let pattern = regex::Regex::new(
        r"(?ix)^([a-z0-9:][a-z0-9.:-]{0,254})\s*(?:の\s*|\s+)((?:(?:tcp|udp)\s*)?(?:ポート|port)\s*[a-z0-9_-]+|(?:[a-z0-9_-]+\s*/\s*(?:tcp|udp)|(?:tcp|udp)\s*/\s*[a-z0-9_-]+)|[a-z][a-z0-9_-]*)\s*(?:(?:(?:が|は)\s*(?:空いている|開いている|開放されている|接続できる|通る)\s*か|(?:の|への)?\s*(?:疎通|接続|開放)(?:確認)?)?\s*(?:(?:を\s*)?(?:チェック(?:して)?|確認(?:して)?|調べて|テスト(?:して)?|check|test))|(?:って|は|が)?\s*(?:空いて(?:います|ます|いる)?|開いて(?:います|ます|いる)?|開放されて(?:います|いる)?|接続でき(?:ます|る)?|通る)\s*(?:か|でしょうか))[？?。！!]*$"
    ).ok()?;
    let captures = pattern.captures(message.trim())?;
    let host = captures.get(1)?.as_str();
    let spec = captures.get(2)?.as_str();
    let label = regex::Regex::new(r"(?i)^(?:(tcp|udp)\s*)?(?:ポート|port)\s*([a-z0-9_-]+)$").ok()?;
    let resolved = if let Some(parts) = label.captures(spec) {
        crate::service_ports::resolve(&parts[2], parts.get(1).map(|p| p.as_str())).ok()?
    } else {
        crate::service_ports::resolve(spec, None).ok()?
    };
    if resolved.protocol != "tcp" { return None; }
    let port = resolved.port;
    Some(LegacyShortcut {
        tool: Some("self_network_test_connection".into()), target: Some("localhost".into()),
        args: serde_json::json!({"host":host,"port":port,"protocol":"tcp"}), reply: None,
    })
}

/// Match only a complete, read-only request for this computer's routes.
/// `scope` is shared by desktop and CLI; absent scope retains the legacy default.
pub fn local_route_shortcut(message: &str) -> Option<LegacyShortcut> {
    let pattern = regex::Regex::new(
        r"(?ix)^(?:localhost|127\.0\.0\.1|::1|local|ローカル|自機|このpc|このコンピュータ|この端末)\s*(?:の\s*|\s+)(?:(?:ipv4\s*)?(?:デフォルトルート|デフォルトゲートウェイ|default\s+(?:route|gateway))|ルーティング(?:テーブル)?|経路(?:表|テーブル)?|routing\s+table|routes?)\s*(?:(?:を\s*)?(?:確認(?:して)?|取得(?:して)?|表示(?:して)?|見せて|教えて|調べて)|は\s*(?:どこ|何|なに)(?:ですか)?|一覧|show|list)?[？?。！!]*$",
    ).ok()?;
    if !pattern.is_match(message.trim()) {
        return None;
    }
    let lower = message.to_ascii_lowercase();
    let scope = if lower.contains("デフォルト") || lower.contains("default") {
        "default"
    } else {
        "table"
    };
    Some(LegacyShortcut {
        tool: Some("self_network_route".into()),
        target: Some("localhost".into()),
        args: serde_json::json!({"scope": scope}),
        reply: None,
    })
}

/// Resolve an explicit local next-hop question through the kernel routing table.
/// Accept IP literals only, so arbitrary text can never become command arguments.
pub fn local_next_hop_shortcut(message: &str) -> Option<LegacyShortcut> {
    let pattern = regex::Regex::new(
        r"(?ix)^(?:localhost|127\.0\.0\.1|::1|local|ローカル|自機|このpc|このコンピュータ|この端末)\s*(?:の\s*|\s+)([0-9a-f:.]+)\s*(?:への?\s*|の\s*|\s+)(?:ネクストホップ|次ホップ|next\s*hop|経路|ルート|route)\s*(?:(?:は\s*)?(?:どこ|何|なに)(?:ですか)?|(?:を\s*)?(?:教えて|確認して|調べて|表示して))?[？?。！!]*$",
    ).ok()?;
    let captures = pattern.captures(message.trim())?;
    let destination = captures.get(1)?.as_str().parse::<std::net::IpAddr>().ok()?;
    Some(LegacyShortcut {
        tool: Some("self_network_route".into()), target: Some("localhost".into()),
        args: serde_json::json!({"scope":"destination", "destination":destination.to_string()}), reply: None,
    })
}

pub fn is_explanatory_request(message: &str) -> bool {
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
pub fn is_configuration_change_request(message: &str) -> bool {
    crate::intent::is_configuration_change_request(message)
}

/// Extracts the first bounded MAC address and returns its canonical lowercase
/// colon form. Kept in core so runtimes can share old deterministic lookups.
pub fn mac_address_in_goal(goal: &str) -> Option<String> {
    let pattern = regex::Regex::new(
        r"(?i)(?:[0-9a-f]{1,2}[:-]){5}[0-9a-f]{1,2}|(?:[0-9a-f]{4}\.){2}[0-9a-f]{4}",
    )
    .ok()?;
    let address = pattern.find_iter(goal).find_map(|found| {
        let before = goal[..found.start()].chars().last();
        let after = goal[found.end()..].chars().next();
        if before.is_some_and(|ch| ch.is_ascii_hexdigit() || matches!(ch, ':' | '-' | '.'))
            || after.is_some_and(|ch| ch.is_ascii_hexdigit() || matches!(ch, ':' | '-' | '.'))
        {
            return None;
        }
        let raw = found.as_str();
        if raw.contains('.') {
            let digits = raw.replace('.', "").to_ascii_lowercase();
            return Some(
                (0..6)
                    .map(|index| &digits[index * 2..index * 2 + 2])
                    .collect::<Vec<_>>()
                    .join(":"),
            );
        }
        Some(
            raw.split([':', '-'])
                .map(|octet| format!("{octet:0>2}").to_ascii_lowercase())
                .collect::<Vec<_>>()
                .join(":"),
        )
    });
    address
}

pub fn arp_mac_target(goal: &str) -> Option<String> {
    goal.to_ascii_lowercase()
        .contains("arp")
        .then(|| mac_address_in_goal(goal))
        .flatten()
}

pub fn local_arp_mac_target(goal: &str) -> Option<String> {
    let lower = goal.to_ascii_lowercase();
    let local = [
        "localhost",
        "127.0.0.1",
        "::1",
        "local",
        "ローカル",
        "自機",
        "このpc",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    (local && lower.contains("arp"))
        .then(|| mac_address_in_goal(goal))
        .flatten()
}

pub fn mac_lookup_requires_ping(goal: &str) -> bool {
    goal.contains("応答") || goal.contains("疎通") || goal.to_ascii_lowercase().contains("ping")
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyShortcut {
    pub tool: Option<String>,
    pub target: Option<String>,
    pub args: serde_json::Value,
    pub reply: Option<String>,
}

/// Preserve the old deterministic fastroute behavior without a UI/runtime
/// dependency. Returns only actions whose intent is explicit in the request.
pub fn legacy_shortcut(goal: &str) -> Option<LegacyShortcut> {
    if let Some(shortcut) = explicit_probe_shortcut(goal) {
        return Some(shortcut);
    }
    if let Some(shortcut) = port_check_shortcut(goal) {
        return Some(shortcut);
    }
    let normalized = goal.trim();
    let lower = normalized.to_ascii_lowercase();
    let shortcut = |tool: &str, target: Option<&str>, args: serde_json::Value| LegacyShortcut {
        tool: Some(tool.to_string()),
        target: target.map(str::to_string),
        args,
        reply: None,
    };

    if is_greeting(normalized) {
        return Some(LegacyShortcut {
            tool: None,
            target: None,
            args: serde_json::Value::Null,
            reply: Some("こんにちは！私はMIKOMAIです。ネットワークインフラの診断、運用、トラブルシューティングを支援します。機器の状態確認、PingやTraceroute、NW-DB検索、ログ分析などをお手伝いできます。何を確認しましょうか？".into()),
        });
    }
    if let Some(start) = lower.find("nwdiag {") {
        let diagram = normalized[start..].trim();
        return Some(shortcut(
            "self_network_nwdiag",
            None,
            serde_json::json!({"schema":crate::plotter::extract_schema(diagram).unwrap_or(diagram)}),
        ));
    }
    if lower.contains("dhcprequest")
        || lower.contains("dhcp request")
        || lower.contains("dhcpリクエスト")
        || lower.contains("dhcp要求")
    {
        let prepare = ["preview", "prepare", "作成", "生成", "プレビュー", "準備"]
            .iter()
            .any(|term| lower.contains(term));
        return Some(shortcut(
            "network_packet_safety",
            None,
            serde_json::json!({"intent": if prepare {"prepare_dhcp_request"} else {"dhcp_request_probe"}}),
        ));
    }
    if lower.contains("test-netconnection") || lower.contains("tnc ") {
        let host = regex::Regex::new(r"(?i)(?:-computername\s+|tnc\s+)([a-z0-9_.:-]+)")
            .ok()?
            .captures(normalized)?
            .get(1)?
            .as_str();
        let port = regex::Regex::new(r"(?i)-port\s+(\d+)")
            .ok()?
            .captures(normalized)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<u16>().ok());
        return Some(shortcut(
            "self_network_test_connection",
            None,
            serde_json::json!({"host":host,"port":port}),
        ));
    }
    if lower.contains("traceroute")
        || lower.contains("trace")
        || lower.contains("トレースルート")
    {
        let host = regex::Regex::new(
            r"(?i)(?:traceroute|trace\s+route|trace|トレースルート)\s*(?::|=|：)?\s*([a-z0-9_.:-]+)",
        )
        .ok()?
        .captures(normalized)?
        .get(1)?
        .as_str();
        return Some(shortcut(
            "self_network_traceroute",
            None,
            serde_json::json!({"host":host}),
        ));
    }
    if (lower.contains("ping") || lower.contains("ピング") || lower.contains("ピン "))
        && ![
            "とは",
            "方法",
            "どうやって",
            "できますか",
            "可能ですか",
            "?",
            "？",
        ]
        .iter()
        .any(|term| lower.contains(term))
    {
        let regex = regex::Regex::new(r"(?i)(?:ping|ピング|ピン)\s*(?::|=|：)?\s*([a-z0-9_.:-]+)|([a-z0-9_.:-]+)\s*(?:に|へ|で|を)?\s*(?:ping|ピング|ピン)").ok()?;
        let captures = regex.captures(normalized)?;
        let host = captures.get(1).or_else(|| captures.get(2))?.as_str();
        let count = regex::Regex::new(r"(?i)(?:count|回数|回)\s*(\d+)|(\d+)\s*回(?:実行)?")
            .ok()?
            .captures(normalized)
            .and_then(|c| c.get(1).or_else(|| c.get(2)))
            .and_then(|m| m.as_str().parse::<u32>().ok());
        let size = regex::Regex::new(r"(?i)(?:size|サイズ)\s*(\d+)")
            .ok()?
            .captures(normalized)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<u32>().ok());
        let dont_fragment = ["df", "don't fragment", "dont fragment", "フラグメント禁止"]
            .iter()
            .any(|term| lower.contains(term));
        return Some(shortcut(
            "self_network_ping",
            None,
            serde_json::json!({"host":host,"count":count,"size":size,"dont_fragment":dont_fragment}),
        ));
    }
    if lower.contains("arp")
        && ["localhost", "ローカル", "自機", "このpc", "local"]
            .iter()
            .any(|marker| lower.contains(marker))
    {
        return Some(shortcut(
            "get_state",
            Some("localhost"),
            serde_json::json!({"device":"localhost","resource":"arp"}),
        ));
    }
    if let Some(route) = local_route_shortcut(goal) {
        return Some(route);
    }
    if (lower.contains("console") || lower.contains("コンソール") || lower.contains("シリアル"))
        && ["list", "一覧", "ポート", "リスト"]
            .iter()
            .any(|marker| lower.contains(marker))
    {
        return Some(shortcut(
            "network_list_serial_ports",
            None,
            serde_json::json!({}),
        ));
    }
    None
}

fn is_greeting(message: &str) -> bool {
    let lower = message.trim().trim_end_matches(['!', '！', '?', '？', '。', '.', '〜', '～']).trim().to_ascii_lowercase();
    [
        "やっほー",
        "やっほ",
        "やほー",
        "やほ",
        "こんにちは",
        "こんにちわ",
        "はじめまして",
        "おはよう",
        "おはようございます",
        "こんばんは",
        "お疲れ様",
        "お疲れ様です",
        "おつかれさま",
        "ハロー",
        "はろー",
        "hello",
        "hi",
        "hey",
    ]
    .iter()
    .any(|greeting| lower == *greeting)
        || [
            "自己紹介",
            "じこしょうかい",
            "who are you",
            "あなたは",
            "お名前",
            "なまえ",
            "名前",
            "何ができますか",
            "なにができますか",
        ]
        .iter()
        .any(|term| lower.contains(term))
}

pub fn select_dispatch_mode(message: &str) -> DispatchMode {
    if crate::attachment_transfer::upload_protocol(message).is_some() {
        return DispatchMode::Agent;
    }
    if crate::network::interface_check::request(message).is_some() {
        return DispatchMode::Agent;
    }
    if message.starts_with("__MIKOMAI_RESUME__") {
        return DispatchMode::Agent;
    }
    let normalized = message.to_lowercase();
    if is_configuration_change_request(&normalized) {
        return DispatchMode::Agent;
    }
    if local_next_hop_shortcut(message).is_some() {
        return DispatchMode::Agent;
    }
    if fast_route(message).is_some() {
        return DispatchMode::FastRouter;
    }
    if crate::network::ndp::is_local_request(message) {
        return DispatchMode::Agent;
    }
    if crate::plotter::is_diagram_request(message) || legacy_shortcut(message).is_some() {
        return DispatchMode::Agent;
    }
    let live = [
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
        "疎通",
        "接続確認",
        "ポート",
        "/tcp",
        "/udp",
        "port",
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
        "繋がらない",
        "つながらない",
        "接続できない",
        "通信できない",
        "通信不可",
        "届かない",
        "通らない",
        "アクセスできない",
        "不通",
        "切断",
        "パケットロス",
        "パケロス",
        "タイムアウト",
        "timeout",
        "unreachable",
        "cannot connect",
        "failed",
        "トラブル",
        "エラー",
        "異常",
    ]
    .iter()
    .any(|m| normalized.contains(m));
    if live && !is_explanatory_request(&normalized) {
        DispatchMode::Agent
    } else {
        DispatchMode::Worker
    }
}

/// Uses a caller-provided inventory to resolve context without depending on
/// a desktop runtime or a secrets-bearing device registry.
pub fn select_dispatch_mode_for_devices(
    message: &str,
    registered_devices: &[String],
) -> DispatchMode {
    let mode = select_dispatch_mode(message);
    if mode != DispatchMode::Worker || is_explanatory_request(&message.to_lowercase()) {
        return mode;
    }
    if registered_devices.iter().any(|device| {
        !device.trim().is_empty() && message.to_lowercase().contains(&device.to_lowercase())
    }) {
        DispatchMode::Agent
    } else {
        DispatchMode::Worker
    }
}
#[cfg(test)]
mod tests {

    #[test]
    fn service_port_checks_resolve_names_and_preserve_tcp_only_execution() {
        for (goal, port) in [("router のsshを確認して",22),("router のDNSが開いているかチェック",53),("router のtcp/22をチェック",22),("router のTCPポートhttpsを確認",443),("router のdns/tcpをチェック",53)] {
            let route = super::fast_route(goal).unwrap_or_else(|| panic!("{goal}"));
            assert_eq!(route.args["port"], port);
            assert_eq!(route.args["protocol"], "tcp");
        }
        for goal in ["router のudp/22をチェック", "router のdns/udpを確認", "router のntpをチェック", "router のunknownを確認", "router のsshとhttpsを確認", "router のsshの設定方法を教えて"] {
            assert!(super::fast_route(goal).is_none(), "{goal}");
        }
    }

    #[test]
    fn port_checks_are_complete_tcp_requests_only() {
        for goal in ["NakaokuGW の22/tcpが空いているかチェック", "NakaokuGW  の22/tcpって空いてますか？", "NakaokuGWの22/tcpは開いていますか？", "192.0.2.1のTCPポート443を確認して", "::1 の80/tcpをチェック", "router-1 のポート65535を確認"] {
            let route = super::fast_route(goal).expect(goal);
            assert_eq!(route.tool.as_deref(), Some("self_network_test_connection"));
            assert_eq!(super::select_dispatch_mode(goal), super::DispatchMode::FastRouter);
        }
        assert_eq!(super::fast_route("NakaokuGW の22/tcpが空いているかチェック").unwrap().args["port"], 22);
        for goal in ["NakaokuGW の22/tcpが空いているかチェックして、Pingも実行して", "router のポート22と443を確認して", "router のポート0を確認", "router のポート65536を確認", "router の53/udpをチェック", "router の22/tcpの確認方法を教えて"] {
            assert!(super::fast_route(goal).is_none(), "{goal}");
        }
        assert_eq!(super::select_dispatch_mode("NakaokuGW の22/tcpと443/tcpをチェックし、原因も調査"), super::DispatchMode::Agent);
    }

    use super::*;
    #[test]
    fn local_routes_use_live_results_with_explicit_scope() {
        for (goal, scope) in [
            ("localhost のデフォルトルートはどこ？", "default"),
            ("localhost のデフォルトルートを教えて", "default"),
            ("自機のデフォルトゲートウェイを教えて", "default"),
            ("このPCのルーティングを確認して", "table"),
            ("localhost routes", "table"),
            ("127.0.0.1 の経路表を表示して", "table"),
        ] {
            let shortcut = fast_route(goal).unwrap();
            assert_eq!(shortcut.tool.as_deref(), Some("self_network_route"));
            assert_eq!(shortcut.target.as_deref(), Some("localhost"));
            assert_eq!(shortcut.args["scope"], scope);
            assert_eq!(select_dispatch_mode(goal), DispatchMode::FastRouter);
        }
        for goal in [
            "localhost のデフォルトルートを変更して",
            "localhost のデフォルトルートとは",
            "localhost のデフォルトルートの設定例",
            "localhost のデフォルトルートを確認して ping 8.8.8.8",
            "router-local のデフォルトルートはどこ？",
            "localhost.example.com routes",
            "R1のルーティングを確認して",
        ] {
            assert!(local_route_shortcut(goal).is_none(), "{goal}");
            assert!(fast_route(goal).is_none(), "{goal}");
        }
    }

    #[test]
    fn fast_router_requires_a_complete_unambiguous_command() {
        for command in ["traceroute 8.8.8.8", "trace route example.com", "trace 8.8.8.8", "TRACE 192.0.2.1", "ping 127.0.0.1 count 3 size 64 df", "tnc example.com -port 443"] {
            assert!(fast_route(command).is_some(), "{command}");
            assert_eq!(select_dispatch_mode_for_devices(command, &["8.8.8.8".into()]), DispatchMode::FastRouter);
        }
        for command in ["traceroute 8.8.8.8 の結果を分析して", "R1から traceroute 8.8.8.8", "tracerouteとは", "traceroute 8.8.8.8; ping 1.1.1.1", "ping fe80::1%en0", "ping 8.8.8.8 と 1.1.1.1", "ping 8.8.8.8 count 999999999999", "ping 8.8.8.8 count 3 count 4", "tnc example.com -port 0", "tnc example.com -port 65536", "__MIKOMAI_RESUME__id\ntraceroute 8.8.8.8"] {
            assert!(fast_route(command).is_none(), "{command}");
            assert_ne!(select_dispatch_mode(command), DispatchMode::FastRouter);
        }
        assert_eq!(fast_route("ping df.example.com").unwrap().args["dont_fragment"], false);
    }
    #[test]
    fn probe_destination_precedes_options() {
        for (input, host, tool) in [
            ("ping -df 5000 8.8.8.8", "8.8.8.8", "self_network_ping"),
            ("ping -unknown 5000 example.com", "example.com", "self_network_ping"),
            ("ping -df 5000 NakaokuGW", "NakaokuGW", "self_network_ping"),
            ("ping 8.8.8.8 -unknown 5000", "8.8.8.8", "self_network_ping"),
            ("trace -m 5 192.0.2.1", "192.0.2.1", "self_network_traceroute"),
            ("trace route -w 2 example.com", "example.com", "self_network_traceroute"),
            ("ping -x label 2001:db8::1", "2001:db8::1", "self_network_ping"),
        ] {
            for parsed in [fast_route(input), legacy_shortcut(input)] {
                let parsed = parsed.expect(input);
                assert_eq!(parsed.args["host"], host);
                assert_eq!(parsed.tool.as_deref(), Some(tool));
            }
        }
        assert_eq!(fast_route("ping -df 5000 8.8.8.8").unwrap().args["size"], serde_json::Value::Null);
        let options = fast_route("ping -c 3 -s 5000 -df 8.8.8.8").unwrap();
        assert_eq!(options.args["count"], 3);
        assert_eq!(options.args["size"], 5000);
        assert_eq!(options.args["dont_fragment"], true);
        assert!(fast_route("ping -df 5000").is_none());
        assert!(fast_route("trace -m 5").is_none());
    }

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
        assert_eq!(
            select_dispatch_mode("NakaokuGW の設定を確認して"),
            DispatchMode::Agent
        );
        assert_eq!(
            select_dispatch_mode("Yamahaの設定例を作成して"),
            DispatchMode::Worker
        );
    }

    #[test]
    fn routes_current_device_requests_to_agent_without_gui_context() {
        let devices = vec!["router-1".to_string()];
        assert_eq!(
            select_dispatch_mode_for_devices("router-1の状態", &devices),
            DispatchMode::Agent
        );
        assert_eq!(
            select_dispatch_mode_for_devices("router-1の設定例", &devices),
            DispatchMode::Worker
        );
    }

    #[test]
    fn canonicalizes_legacy_arp_mac_shortcut_targets() {
        assert_eq!(
            arp_mac_target("NakaokuGW ARP 00aa.bbcc.ddee"),
            Some("00:aa:bb:cc:dd:ee".into())
        );
        assert_eq!(
            local_arp_mac_target("localhost ARP ea-f1-92-50-7b-c3"),
            Some("ea:f1:92:50:7b:c3".into())
        );
        assert_eq!(local_arp_mac_target("router ARP ea:f1:92:50:7b:c3"), None);
    }

    #[test]
    fn preserves_legacy_fastroute_shortcuts_as_portable_decisions() {
        let ping = legacy_shortcut("ping 192.0.2.10 count 3 size 64").unwrap();
        assert_eq!(ping.tool.as_deref(), Some("self_network_ping"));
        assert_eq!(ping.args["host"], "192.0.2.10");
        assert_eq!(ping.args["count"], 3);
        assert_eq!(ping.args["size"], 64);

        let serial = legacy_shortcut("console port一覧").unwrap();
        assert_eq!(serial.tool.as_deref(), Some("network_list_serial_ports"));

        let greeting = legacy_shortcut("こんにちは").unwrap();
        assert!(greeting
            .reply
            .as_deref()
            .is_some_and(|reply| reply.contains("MIKOMAI")));

        let change_request = "F220にVLANを設定する";
        assert!(legacy_shortcut(change_request).is_none());
        assert!(
            legacy_shortcut("DHCPRequestをプレビューして").unwrap().args["intent"]
                == "prepare_dhcp_request"
        );
        assert!(legacy_shortcut("ping 192.0.2.1は可能ですか").is_none());
        assert!(legacy_shortcut("おはようございます")
            .unwrap()
            .reply
            .is_some());
    }
}

#[cfg(test)]
mod next_hop_tests {
    use super::*;
    #[test]
    fn local_next_hops_use_agent_and_preserve_valid_destinations() {
        for (goal, ip) in [
            ("localhost の8.8.8.8 のネクストホップはどこですか？", "8.8.8.8"),
            ("自機の2001:db8::1への経路を教えて", "2001:db8::1"),
        ] {
            assert_eq!(select_dispatch_mode(goal), DispatchMode::Agent);
            assert_eq!(local_next_hop_shortcut(goal).unwrap().args["destination"], ip);
            assert!(fast_route(goal).is_none());
        }
        for goal in ["R1の8.8.8.8のネクストホップはどこ？", "localhost の999.8.8.8の経路はどこ？", "localhost の8.8.8.8;touchの経路はどこ？"] {
            assert!(local_next_hop_shortcut(goal).is_none());
        }
    }
}
