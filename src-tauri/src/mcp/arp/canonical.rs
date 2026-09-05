use crate::mcp::canonicalization::{extract_candidates, ensure_unique, CandidateVectors, EvidenceLine, ExtractedCandidates};
use crate::schema::arp::{ArpEntry, ArpEntryType, ArpMetadata, UniversalArpTable};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArpSelection {
    pub entries: Vec<ArpEntrySelection>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArpEntrySelection {
    pub ip_idx: usize,
    pub mac_idx: Option<usize>,
    pub interface_idx: Option<usize>,
    #[serde(rename = "type")]
    pub entry_type: ArpEntryType,
    pub age_seconds: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArpCanonicalizationEvidence {
    pub candidates: CandidateVectors,
    pub lines: Vec<EvidenceLine>,
}

pub fn extract(raw: &str) -> ExtractedCandidates {
    extract_candidates(raw, |line| {
        // ARP CLIs conventionally place the outgoing interface in the final
        // column. Require an IP and MAC on the line to avoid header tokens.
        let words = line.split_whitespace().collect::<Vec<_>>();
        if words.len() < 3 || !line.contains('.') { return None; }
        let last = *words.last()?;
        if last.eq_ignore_ascii_case("incomplete") || last.parse::<u32>().is_ok() || last.contains('.') || last.contains(':') || last.contains('-') || crate::mcp::canonicalization::normalize_mac(last).len() == 17 { return None; }
        Some(last.trim_matches(|c: char| c == ',' || c == ';').to_string())
    })
}

pub fn prompt_contract(extracted: &ExtractedCandidates, raw: &str) -> String {
    format!(
        r#"Return YAML only, exactly this shape:
entries:
  - ip_idx: 0
    mac_idx: 0
    interface_idx: 0
    type: dynamic
    age_seconds: 0

Rules:
- ip_idx MUST be an integer index into IP candidates.
- mac_idx MUST be an integer index into MAC candidates, or null when no MAC exists.
- interface_idx MUST be an integer index into Interface candidates, or null when no interface exists.
- Do not emit IP, MAC, or interface strings directly.
- age_seconds MUST be a non-negative integer or null.
- type MUST be one of: dynamic, static, incomplete, permanent.
- Use the Raw CLI only to determine relationships and scalar attributes.
- Emit each ARP relationship exactly once.
- Never invent values or relationships that are not supported by the Raw CLI.

IP candidates: {:?}
MAC candidates: {:?}
Interface candidates: {:?}
Evidence lines: {:?}
Raw CLI:
{}"#,
        extracted.candidates.ip_addresses,
        extracted.candidates.mac_addresses,
        extracted.candidates.interfaces,
        extracted.evidence,
        raw
    )
}

pub fn reconstruct_and_validate(selection: ArpSelection, extracted: &ExtractedCandidates, device_name: &str, os_type: &str, generated_at: DateTime<Utc>) -> Result<UniversalArpTable, String> {
    if selection.entries.is_empty() { return Err("ARP selection contains no entries".to_string()); }
    ensure_unique(selection.entries.iter().map(|entry| entry.ip_idx), "IP index")?;
    let expected_ips = extracted.evidence.iter()
        .flat_map(|line| line.ip_indexes.iter().copied())
        .collect::<std::collections::HashSet<_>>();
    let selected_ips = selection.entries.iter().map(|entry| entry.ip_idx)
        .collect::<std::collections::HashSet<_>>();
    if expected_ips != selected_ips {
        return Err("ARP selection has missing or unexpected IP relationships".to_string());
    }
    let mut entries = Vec::with_capacity(selection.entries.len());
    for selected in selection.entries {
        let ip_address = extracted.candidates.ip_addresses.get(selected.ip_idx).ok_or_else(|| format!("ip_idx {} is outside extracted candidates", selected.ip_idx))?.clone();
        if selected.entry_type != ArpEntryType::Incomplete && selected.mac_idx.is_none() { return Err("non-incomplete ARP entry requires mac_idx".to_string()); }
        let mac_address = selected.mac_idx.map(|index| extracted.candidates.mac_addresses.get(index).ok_or_else(|| format!("mac_idx {} is outside extracted candidates", index)).cloned()).transpose()?;
        let interface = selected.interface_idx.map(|index| extracted.candidates.interfaces.get(index).ok_or_else(|| format!("interface_idx {} is outside extracted candidates", index)).cloned()).transpose()?;
        let matching_lines = extracted.evidence.iter().filter(|line| line.ip_indexes.contains(&selected.ip_idx) && selected.mac_idx.map_or(true, |index| line.mac_indexes.contains(&index)) && selected.interface_idx.map_or(true, |index| line.interface_indexes.contains(&index))).collect::<Vec<_>>();
        let co_occurs = !matching_lines.is_empty();
        if !co_occurs { return Err(format!("selected ARP relationship does not co-occur on a raw evidence line (ip_idx={}, mac_idx={:?}, interface_idx={:?})", selected.ip_idx, selected.mac_idx, selected.interface_idx)); }
        if let Some(age) = selected.age_seconds {
            if !matching_lines.iter().any(|line| line.scalar_values.contains(&age)) { return Err(format!("age_seconds {age} is not present on the selected raw evidence line")); }
        } else if matching_lines.iter().any(|line| !line.scalar_values.is_empty()) {
            return Err("age_seconds is null but the selected raw evidence line has scalar values".to_string());
        }
        entries.push(ArpEntry { ip_address, mac_address, r#type: selected.entry_type, interface, age_seconds: selected.age_seconds });
    }
    let table = UniversalArpTable { version: "1.0".to_string(), metadata: ArpMetadata { generated_at, source_device: device_name.to_string(), os_type: os_type.to_string() }, arp_table: entries };
    table.validate().map_err(|error| format!("canonical ARP schema validation failed: {error}"))?;
    Ok(table)
}

pub fn evidence(extracted: &ExtractedCandidates) -> ArpCanonicalizationEvidence {
    ArpCanonicalizationEvidence { candidates: extracted.candidates.clone(), lines: extracted.evidence.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_reconstructs_and_rejects_non_cooccurring_candidates() {
        let raw = "Protocol Address Age Hardware Addr Type Interface\nInternet 192.0.2.1 2 0011.2233.4455 ARPA Gi1/0/1\nInternet 192.0.2.2 3 00aa.bbcc.ddee ARPA Gi1/0/2";
        let extracted = extract(raw);
        assert_eq!(extracted.candidates.ip_addresses, ["192.0.2.1", "192.0.2.2"]);
        assert_eq!(extracted.candidates.mac_addresses, ["00:11:22:33:44:55", "00:aa:bb:cc:dd:ee"]);
        let table = reconstruct_and_validate(ArpSelection { entries: vec![ArpEntrySelection { ip_idx: 0, mac_idx: Some(0), interface_idx: Some(0), entry_type: ArpEntryType::Dynamic, age_seconds: Some(2) }, ArpEntrySelection { ip_idx: 1, mac_idx: Some(1), interface_idx: Some(1), entry_type: ArpEntryType::Dynamic, age_seconds: Some(3) }] }, &extracted, "r1", "ios", Utc::now()).unwrap();
        assert_eq!(table.arp_table[0].interface.as_deref(), Some("Gi1/0/1"));
        let error = reconstruct_and_validate(ArpSelection { entries: vec![ArpEntrySelection { ip_idx: 0, mac_idx: Some(1), interface_idx: Some(0), entry_type: ArpEntryType::Dynamic, age_seconds: Some(2) }, ArpEntrySelection { ip_idx: 1, mac_idx: Some(1), interface_idx: Some(1), entry_type: ArpEntryType::Dynamic, age_seconds: Some(3) }] }, &extracted, "r1", "ios", Utc::now()).unwrap_err();
        assert!(error.contains("co-occur"));
    }

    #[test]
    fn keeps_ttl_scalar_out_of_interfaces_and_allows_incomplete() {
        let raw = "192.0.2.10 1004 0011.2233.4455 dynamic LAN1\n192.0.2.11 (incomplete)";
        let extracted = extract(raw);
        assert_eq!(extracted.candidates.interfaces, ["LAN1"]);
        let table = reconstruct_and_validate(ArpSelection { entries: vec![
            ArpEntrySelection { ip_idx: 0, mac_idx: Some(0), interface_idx: Some(0), entry_type: ArpEntryType::Dynamic, age_seconds: Some(1004) },
            ArpEntrySelection { ip_idx: 1, mac_idx: None, interface_idx: None, entry_type: ArpEntryType::Incomplete, age_seconds: None },
        ] }, &extracted, "r1", "yamaha", Utc::now()).unwrap();
        assert_eq!(table.arp_table[1].mac_address, None);
        assert_eq!(table.arp_table[1].interface, None);
    }
}
