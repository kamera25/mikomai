//! Constrained canonicalization for interface status/address output.

use crate::mcp::canonicalization::{
    ensure_unique, extract_candidates, CandidateVectors, EvidenceLine, ExtractedCandidates,
};
use crate::schema::interface::{
    InterfaceEntry, InterfaceMetadata, InterfaceStatus, UniversalInterfaceTable,
};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use validator::Validate;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InterfaceSelection {
    pub entries: Vec<InterfaceEntrySelection>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InterfaceEntrySelection {
    pub line_idx: usize,
    pub name_idx: usize,
    pub status: InterfaceStatus,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub ip_idxs: Vec<usize>,
    pub prefix_len: Option<u8>,
}

/// Treat a missing or explicitly null IP index list as an empty list.
///
/// The canonical selection contract uses `[]` for the normal representation,
/// but accepting `null` makes the constrained LLM response tolerant of the
/// common "no IP address" representation without changing the internal type.
fn deserialize_null_as_empty_vec<'de, D>(deserializer: D) -> Result<Vec<usize>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<Vec<usize>>::deserialize(deserializer).map(|value| value.unwrap_or_default())
}

#[derive(Debug, Clone, Serialize)]
pub struct InterfaceCanonicalizationEvidence {
    pub candidates: CandidateVectors,
    pub lines: Vec<EvidenceLine>,
}

fn interface_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(?:gigabitethernet|fastethernet|ethernet|port-channel|loopback|tunnel|vlan|ge-|xe-|et-|lan|wan)(?:[\w/-]+)?\b").unwrap())
}

fn interface_from_line(line: &str) -> Option<String> {
    interface_re()
        .find_iter(line)
        .map(|m| {
            m.as_str()
                .trim_matches(|c: char| matches!(c, ',' | ';' | ':'))
                .to_string()
        })
        .find(|value| {
            !matches!(
                value.to_ascii_lowercase().as_str(),
                "interface" | "ethernet"
            )
        })
}

fn is_command_marker(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("=== Command:") && trimmed.ends_with("===")
}

pub fn extract(raw: &str) -> ExtractedCandidates {
    // Fold indented address/description lines into their interface header so
    // the shared extractor can prove relationships without inventing a block
    // model of its own.
    let mut blocks = Vec::new();
    let mut current = String::new();
    for line in raw.lines() {
        if is_command_marker(line) {
            continue;
        }
        if interface_from_line(line).is_some()
            && line.chars().next().is_some_and(|c| !c.is_whitespace())
        {
            if !current.is_empty() {
                blocks.push(current);
            }
            current = line.trim().to_string();
        } else if !current.is_empty() {
            current.push(' ');
            current.push_str(line.trim());
        }
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    extract_candidates(&blocks.join("\n"), interface_from_line)
}

pub fn prompt_contract(extracted: &ExtractedCandidates, raw: &str) -> String {
    let numbered_evidence = extracted
        .evidence
        .iter()
        .enumerate()
        .map(|(index, line)| {
            format!(
                "  [{index}] raw_line={} text={:?} interface_idxs={:?} ip_idxs={:?}",
                line.line, line.text, line.interface_indexes, line.ip_indexes
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("Return YAML only, exactly this shape:\nentries:\n  - line_idx: 0\n    name_idx: 0\n    status: up\n    ip_idxs: []\n    prefix_len: null\n\nRules:\n- Emit exactly one entry for every interface evidence line. There are exactly {} evidence lines, so entries must contain exactly {} items.\n- line_idx is the zero-based index in the numbered Evidence lines below (not raw_line). Use each index exactly once.\n- name_idx is required and must be an integer; null is never valid.\n- All indexes must refer to the supplied candidate vectors and occur on the selected evidence line.\n- status must be exactly one of up, down, or unknown.\n- Use up only when the selected evidence line contains an operational/link-up token such as 'is up', 'up/up', or 'connected'.\n- Use down only when the selected evidence line contains a link-down token such as 'is down', 'administratively down', or 'disconnected'.\n- Use unknown when neither up nor down is explicitly present on the selected evidence line.\n- ip_idxs must be [] when the selected evidence line has no IP address; null is also accepted as equivalent to [].\n- Never invent values or relationships; preserve the raw spelling.\n\nInterface candidates: {:?}\nIP candidates: {:?}\nNumbered Evidence lines:\n{}\nRaw CLI:\n{}", extracted.evidence.len(), extracted.evidence.len(), extracted.candidates.interfaces, extracted.candidates.ip_addresses, numbered_evidence, raw)
}

pub fn reconstruct_and_validate(
    selection: InterfaceSelection,
    extracted: &ExtractedCandidates,
    device_name: &str,
    os_type: &str,
    generated_at: DateTime<Utc>,
) -> Result<UniversalInterfaceTable, String> {
    if selection.entries.is_empty() {
        return Err("interface selection contains no entries".to_string());
    }
    ensure_unique(
        selection.entries.iter().map(|entry| entry.line_idx),
        "interface evidence line index",
    )?;
    if selection.entries.len() != extracted.evidence.len() {
        let selected_indexes = selection
            .entries
            .iter()
            .map(|entry| entry.line_idx.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let evidence_summary = extracted
            .evidence
            .iter()
            .enumerate()
            .map(|(index, line)| format!("[{index}] raw_line={} {:?}", line.line, line.text))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!(
            "interface selection has missing or unexpected relationships: expected {} entries, received {}; selected line_idx=[{}]; evidence={}",
            extracted.evidence.len(),
            selection.entries.len(),
            selected_indexes,
            evidence_summary
        ));
    }
    let mut interfaces = Vec::with_capacity(selection.entries.len());
    for selected in selection.entries {
        let line = extracted.evidence.get(selected.line_idx).ok_or_else(|| {
            format!(
                "line_idx {} is outside interface evidence",
                selected.line_idx
            )
        })?;
        if !line.interface_indexes.contains(&selected.name_idx) {
            return Err(format!(
                "name_idx {} is not on evidence line {}",
                selected.name_idx, line.line
            ));
        }
        let lower = line.text.to_ascii_lowercase();
        let has_up = lower
            .split(|character: char| !character.is_ascii_alphabetic())
            .any(|token| token == "up" || token == "connected");
        let has_down = lower
            .split(|character: char| !character.is_ascii_alphabetic())
            .any(|token| token == "down" || token == "disconnected");
        let status_supported = match selected.status {
            InterfaceStatus::Up => has_up,
            InterfaceStatus::Down => has_down,
            InterfaceStatus::Unknown => !has_up && !has_down,
        };
        if !status_supported {
            return Err(format!(
                "status {:?} is not supported by evidence line {}",
                selected.status, line.line
            ));
        }
        if selected.ip_idxs != line.ip_indexes {
            return Err(format!(
                "ip_idxs must select every IP on evidence line {}",
                line.line
            ));
        }
        if selected
            .ip_idxs
            .iter()
            .any(|idx| !line.ip_indexes.contains(idx))
        {
            return Err(format!(
                "an IP selection is not on evidence line {}",
                line.line
            ));
        }
        if let Some(prefix) = selected.prefix_len {
            if prefix > 32 {
                return Err("prefix_len must be between 0 and 32".to_string());
            }
            if !line.text.contains(&format!("/{prefix}")) {
                return Err(format!(
                    "prefix_len {prefix} is not present on evidence line {}",
                    line.line
                ));
            }
        }
        let name = extracted
            .candidates
            .interfaces
            .get(selected.name_idx)
            .ok_or_else(|| "interface name index is outside candidates".to_string())?
            .clone();
        let ipv4_addresses = selected
            .ip_idxs
            .iter()
            .map(|idx| {
                extracted
                    .candidates
                    .ip_addresses
                    .get(*idx)
                    .cloned()
                    .ok_or_else(|| format!("ip index {idx} is outside candidates"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        interfaces.push(InterfaceEntry {
            name,
            status: selected.status,
            ipv4_addresses,
            prefix_len: selected.prefix_len,
        });
    }
    let table = UniversalInterfaceTable {
        version: "1.0".to_string(),
        metadata: InterfaceMetadata {
            generated_at: generated_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            source_device: device_name.to_string(),
            os_type: os_type.to_string(),
        },
        interfaces,
    };
    table
        .validate()
        .map_err(|error| format!("canonical interface schema validation failed: {error}"))?;
    Ok(table)
}

pub fn evidence(extracted: &ExtractedCandidates) -> InterfaceCanonicalizationEvidence {
    InterfaceCanonicalizationEvidence {
        candidates: extracted.candidates.clone(),
        lines: extracted.evidence.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_and_validates_interface_relationships() {
        let raw = "GigabitEthernet1/0/1 is up\n  Internet address is 192.0.2.1/24";
        let extracted = extract(raw);
        assert_eq!(extracted.evidence.len(), 1);
        assert_eq!(extracted.candidates.interfaces, ["GigabitEthernet1/0/1"]);
        assert_eq!(extracted.candidates.ip_addresses, ["192.0.2.1"]);
        let table = reconstruct_and_validate(
            InterfaceSelection {
                entries: vec![InterfaceEntrySelection {
                    line_idx: 0,
                    name_idx: 0,
                    status: InterfaceStatus::Up,
                    ip_idxs: vec![0],
                    prefix_len: Some(24),
                }],
            },
            &extracted,
            "r1",
            "ios",
            Utc::now(),
        )
        .unwrap();
        assert_eq!(table.interfaces[0].name, "GigabitEthernet1/0/1");
    }

    #[test]
    fn accepts_null_ip_indexes_when_interface_has_no_ip_address() {
        let raw = "GigabitEthernet1/0/1 is up";
        let extracted = extract(raw);
        let selection = serde_yaml::from_str::<InterfaceSelection>(
            "entries:\n  - line_idx: 0\n    name_idx: 0\n    status: up\n    ip_idxs: null\n    prefix_len: null\n",
        )
        .unwrap();

        let table =
            reconstruct_and_validate(selection, &extracted, "r1", "ios", Utc::now()).unwrap();

        assert!(table.interfaces[0].ipv4_addresses.is_empty());
        assert_eq!(table.interfaces[0].prefix_len, None);
    }

    #[test]
    fn prompt_numbers_evidence_lines_separately_from_raw_line_numbers() {
        let extracted = extract("GigabitEthernet1/0/1 is up\n  Internet address is 192.0.2.1/24");
        let prompt = prompt_contract(&extracted, "raw");

        assert!(prompt.contains("There are exactly 1 evidence lines"));
        assert!(prompt.contains("[0] raw_line=1"));
        assert!(prompt.contains("line_idx is the zero-based index"));
    }

    #[test]
    fn groups_yamaha_status_command_output_by_interface() {
        let raw = "=== Command: show status lan1 ===\nLAN1\nIPアドレス: 192.0.2.1/24\n\n=== Command: show status lan2 ===\nLAN2\nIPアドレス: 192.0.2.2/24\n\n=== Command: show status wan1 ===\nWAN1:\n携帯端末は一度も継っていません";
        let extracted = extract(raw);

        assert_eq!(extracted.evidence.len(), 3);
        assert_eq!(extracted.candidates.interfaces, ["LAN1", "LAN2", "WAN1"]);
        assert_eq!(extracted.candidates.ip_addresses, ["192.0.2.1", "192.0.2.2"]);
        assert!(extracted.evidence.iter().all(|line| {
            !line.text.contains("=== Command:")
        }));
    }
}
