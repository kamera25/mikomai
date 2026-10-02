//! Selected-document policy: observations answer direct probes; manuals remain
//! available for configuration, explanation, and troubleshooting requests.
use std::collections::HashSet;

pub fn needs_selected_references(goal: &str) -> bool {
    // Only complete, recognized direct probes can skip document prefetch.
    // Compound goals and unfamiliar phrasing retain references: a TCP mention
    // alone does not prove that manuals are irrelevant to the rest of the task.
    crate::dispatch::fast_route(goal).is_none()
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
