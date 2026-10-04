//! Selected-document policy: observations answer direct probes; manuals remain
//! available for configuration, explanation, and troubleshooting requests.
use std::collections::HashSet;

pub fn needs_selected_references(goal: &str) -> bool {
    if crate::dispatch::fast_route(goal).is_some()
        || crate::dispatch::local_next_hop_shortcut(goal).is_some()
        || crate::dispatch::legacy_shortcut(goal).is_some_and(|shortcut| shortcut.reply.is_some())
        || crate::plotter::is_diagram_request(goal)
    {
        return false;
    }
    // General conversation has no reason to consult network manuals. Keep
    // technical explanations, investigations and explicit document requests.
    let lower = goal.to_ascii_lowercase();
    if [
        "ネットワーク", "ルータ", "スイッチ", "機器", "ポート", "疎通", "通信",
        "接続", "設定", "経路", "障害", "ログ", "インターフェース", "アドレス",
        "パケット", "ファイアウォール", "ゲートウェイ", "サーバ", "帯域",
        "ホスト", "無線", "イーサネット", "セグメント", "プロトコル", "認証",
        "資料", "マニュアル", "手順書", "ナレッジ", "仕様書",
    ].iter().any(|term| lower.contains(term)) {
        return true;
    }
    regex::Regex::new(
        r"(?i)(?-u:\b)(?:show|telnet|lldp|stp|vrrp|ipsec|pppoe|qos|tcp|udp|ip|ipv[46]|ping|traceroute|vlan|lan|wan|ospf|bgp|rip|vrf|vpn|nat|acl|arp|dns|dhcp|ssh|snmp|http|https|ftp|tftp|mtu|mac|ethernet|interface|routing|route|network|router|switch|firewall|config(?:uration)?|manual|documentation|yamaha|cisco|juniper|arista|fortinet|fortigate|fitelnet|furukawa|f220|fx201|fx310|rtx\d+|nvr\d+|s[wr]x\d+)(?-u:\b)|(?:[0-9]{1,3}\.){3}[0-9]{1,3}"
    ).is_ok_and(|pattern| pattern.is_match(&lower))
}

/// Copies of the same document under different paths contribute one body.
/// Preserve its source label and every command; do not summarize or truncate.
pub fn deduplicate_selected_references(material: &str) -> String {
    let pattern = regex::Regex::new(r"(?m)^=== 選択資料: .+ ===\r?$").unwrap();
    let headers = pattern.find_iter(material).collect::<Vec<_>>();
    if headers.is_empty() {
        return material.to_string();
    }
    let mut selected = Vec::new();
    let prefix = material[..headers[0].start()].trim_matches(&['\n', '\r'][..]);
    if !prefix.is_empty() {
        selected.push(prefix.to_string());
    }
    let mut bodies = HashSet::new();
    for (index, header) in headers.iter().enumerate() {
        let end = headers
            .get(index + 1)
            .map(|next| next.start())
            .unwrap_or(material.len());
        let body = material[header.end()..end].replace("\r\n", "\n");
        let body = body.trim_matches(&['\n', '\r'][..]);
        if !body.is_empty() && bodies.insert(body.to_string()) {
            selected.push(format!("{}\n{}", header.as_str().trim_end(), body));
        }
    }
    selected.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direct_checks_skip_manuals_but_explanations_and_investigations_keep_them() {
        for goal in [
            "NakaokuGW の22/tcpって空いてますか？",
            "ping 192.0.2.1",
            "自機のデフォルトルートを確認して",
            "やっほー！",
            "こんにちは。",
            "今日はいい天気ですね",
            "ありがとう",
            "明日の予定を相談したい",
        ] {
            assert!(!needs_selected_references(goal), "{goal}");
        }
        for goal in [
            "NakaokuGW の22/tcpと443/tcpをチェックして結果を比較して",
            "22/tcpを確認してログを分析して",
            "F220のVLAN設定方法を教えて",
            "22/tcpが閉じている原因を調査して",
            "22/tcpを確認する手順を説明して",
            "資料に沿って22/tcpを調べて",
            "TCPの仕組みを教えて",
            "ルータの設定を変更して",
            "やっほー、F220のVLAN設定方法を教えて",
            "F220について教えて",
            "OSPFについて教えて",
            "マニュアルを検索して",
        ] {
            assert!(needs_selected_references(goal), "{goal}");
        }
    }
    #[test]
    fn deduplicates_document_copies_without_removing_distinct_commands() {
        let material = "\n\n=== 選択資料: Guide (nw-docs/guide.md) ===\n# Manual\n```\nvlan 10\n interface 1\n```\n\n=== 選択資料: Guide (/app/Resources/nw-docs/guide.md) ===\n# Manual\n```\nvlan 10\n interface 1\n```\n\n=== 選択資料: Other (other.md) ===\n# Manual\n```\nvlan 20\n interface 2\n```\n";
        let compact = deduplicate_selected_references(material);
        assert_eq!(compact.matches("=== 選択資料:").count(), 2);
        assert_eq!(compact.matches("vlan 10").count(), 1);
        assert!(compact.contains("nw-docs/guide.md") && compact.contains("vlan 20\n interface 2"));
        assert_eq!(deduplicate_selected_references(&compact), compact);
        assert_eq!(
            deduplicate_selected_references("plain knowledge"),
            "plain knowledge"
        );
    }
}
