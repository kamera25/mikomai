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
    pub ip_idxs: Vec<usize>,
    pub prefix_len: Option<u8>,
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

pub fn extract(raw: &str) -> ExtractedCandidates {
    // Fold indented address/description lines into their interface header so
    // the shared extractor can prove relationships without inventing a block
    // model of its own.
    let mut blocks = Vec::new();
    let mut current = String::new();
    for line in raw.lines() {
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
    format!("Return YAML only, exactly this shape:\nentries:\n  - line_idx: 0\n    name_idx: 0\n    status: up\n    ip_idxs: []\n    prefix_len: null\n\nRules:\n- Emit exactly one entry for every interface evidence line.\n- All indexes must refer to the supplied candidate vectors and occur on the selected evidence line.\n- status must be up, down, or unknown.\n- Never invent values or relationships; preserve the raw spelling.\n\nInterface candidates: {:?}\nIP candidates: {:?}\nEvidence lines: {:?}\nRaw CLI:\n{}", extracted.candidates.interfaces, extracted.candidates.ip_addresses, extracted.evidence, raw)
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
        return Err("interface selection has missing or unexpected relationships".to_string());
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
        let status_supported = match selected.status {
            InterfaceStatus::Up => lower.contains(" up") || lower.ends_with("up"),
            InterfaceStatus::Down => lower.contains(" down") || lower.ends_with("down"),
            InterfaceStatus::Unknown => !lower.contains(" up") && !lower.contains(" down"),
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
}
