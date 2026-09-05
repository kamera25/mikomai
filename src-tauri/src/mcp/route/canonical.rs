//! Constrained canonicalization for vendor routing-table output.
//!
//! The model selects only values extracted from a single raw route line.  It
//! cannot manufacture destinations, next hops, interface names, or flags.

use crate::mcp::canonicalization::ensure_unique;
use crate::schema::route::{RouteEntry, RouteMetadata, UniversalRouteTable};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RouteCandidateVectors {
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteEvidenceLine {
    pub line: usize,
    pub text: String,
    pub value_indexes: Vec<usize>,
    pub scalar_values: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedRouteCandidates {
    pub candidates: RouteCandidateVectors,
    pub evidence: Vec<RouteEvidenceLine>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RouteSelection {
    pub entries: Vec<RouteEntrySelection>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RouteEntrySelection {
    /// Index into the extracted evidence lines, not the original CLI line.
    pub line_idx: usize,
    pub destination_idx: usize,
    pub gateway_idx: usize,
    pub interface_idx: usize,
    pub flags_idx: Option<usize>,
    pub metric: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteCanonicalizationEvidence {
    pub candidates: RouteCandidateVectors,
    pub lines: Vec<RouteEvidenceLine>,
}

fn is_ip_or_network_token(token: &str) -> bool {
    let token = token.trim_matches(|c: char| matches!(c, '(' | ')' | ',' | ';' | '[' | ']'));
    let (address, prefix) = token
        .split_once('/')
        .map_or((token, None), |(address, prefix)| (address, Some(prefix)));
    let address = address
        .split_once('%')
        .map_or(address, |(address, _zone)| address);
    if address.parse::<std::net::IpAddr>().is_err() {
        return false;
    }
    prefix.map_or(true, |prefix| {
        prefix.parse::<u8>().is_ok_and(|prefix| {
            match address.parse::<std::net::IpAddr>().unwrap() {
                std::net::IpAddr::V4(_) => prefix <= 32,
                std::net::IpAddr::V6(_) => prefix <= 128,
            }
        })
    })
}

fn looks_like_route_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if lower.contains("gateway of last resort")
        || lower.contains("network destination")
        || lower.contains("active routes")
        || lower.contains("routing tables")
        || lower.starts_with("destination")
        || lower.starts_with("codes:")
        || lower.starts_with("gateway")
    {
        return false;
    }
    lower.split_whitespace().any(|word| word == "default")
        || line.split_whitespace().any(is_ip_or_network_token)
}

fn cleaned_tokens(line: &str) -> Vec<String> {
    line.split_whitespace()
        .map(|token| token.trim_matches(|c: char| matches!(c, '(' | ')' | ',' | ';' | '[' | ']')))
        .filter(|token| !token.is_empty())
        .filter(|token| !matches!(token.to_ascii_lowercase().as_str(), "via" | "is" | "to"))
        .map(ToOwned::to_owned)
        .collect()
}

fn intern(values: &mut Vec<String>, value: String) -> usize {
    if let Some(index) = values.iter().position(|existing| existing == &value) {
        index
    } else {
        values.push(value);
        values.len() - 1
    }
}

pub fn extract(raw: &str) -> ExtractedRouteCandidates {
    let mut candidates = RouteCandidateVectors::default();
    let mut evidence = Vec::new();
    for (line_number, text) in raw.lines().enumerate() {
        if !looks_like_route_line(text) {
            continue;
        }
        let tokens = cleaned_tokens(text);
        if tokens.len() < 2 {
            continue;
        }
        let scalar_values = tokens
            .iter()
            .filter_map(|token| token.parse::<u32>().ok())
            .collect();
        let value_indexes = tokens
            .into_iter()
            .map(|token| intern(&mut candidates.values, token))
            .collect();
        evidence.push(RouteEvidenceLine {
            line: line_number + 1,
            text: text.to_string(),
            value_indexes,
            scalar_values,
        });
    }
    ExtractedRouteCandidates {
        candidates,
        evidence,
    }
}

pub fn prompt_contract(extracted: &ExtractedRouteCandidates, raw: &str) -> String {
    format!(
        r#"Return YAML only, exactly this shape:
entries:
  - line_idx: 0
    destination_idx: 0
    gateway_idx: 1
    interface_idx: 2
    flags_idx: null
    metric: null

Rules:
- Emit one entry for each route that can be mapped unambiguously. Do not emit
  a placeholder for a line whose destination, gateway, or interface is unclear.
- line_idx MUST be an integer index into Route evidence lines, used at most once.
- destination_idx, gateway_idx, interface_idx, and flags_idx MUST index Route value candidates.
- The selected destination, gateway, interface, and optional flags MUST all occur on the selected evidence line.
- flags_idx is null when no flags value exists on the line.
- metric is a non-negative integer present as a standalone scalar on the selected line, or null.
- Never invent values, normalize strings, or select a relationship not supported by the Raw CLI.

Route value candidates: {:?}
Route evidence lines: {:?}
Raw CLI:
{}"#,
        extracted.candidates.values, extracted.evidence, raw
    )
}

fn candidate<'a>(
    extracted: &'a ExtractedRouteCandidates,
    index: usize,
    label: &str,
) -> Result<&'a str, String> {
    extracted
        .candidates
        .values
        .get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("{label} index {index} is outside extracted candidates"))
}

pub fn reconstruct_and_validate(
    selection: RouteSelection,
    extracted: &ExtractedRouteCandidates,
    device_name: &str,
    os_type: &str,
    generated_at: DateTime<Utc>,
) -> Result<UniversalRouteTable, String> {
    if selection.entries.is_empty() {
        return Err("route selection contains no entries".to_string());
    }
    ensure_unique(
        selection.entries.iter().map(|entry| entry.line_idx),
        "route evidence line index",
    )?;
    let mut routes = Vec::with_capacity(selection.entries.len());
    for selected in selection.entries {
        let line = extracted
            .evidence
            .get(selected.line_idx)
            .ok_or_else(|| format!("line_idx {} is outside route evidence", selected.line_idx))?;
        for (label, index) in [
            ("destination_idx", selected.destination_idx),
            ("gateway_idx", selected.gateway_idx),
            ("interface_idx", selected.interface_idx),
        ] {
            if !line.value_indexes.contains(&index) {
                return Err(format!(
                    "selected {label} does not occur on raw route evidence line {}",
                    line.line
                ));
            }
        }
        if let Some(index) = selected.flags_idx {
            if !line.value_indexes.contains(&index) {
                return Err(format!(
                    "selected flags_idx does not occur on raw route evidence line {}",
                    line.line
                ));
            }
        }
        if let Some(metric) = selected.metric {
            if metric < 0 || !line.scalar_values.contains(&(metric as u32)) {
                return Err(format!(
                    "metric {metric} is not present on raw route evidence line {}",
                    line.line
                ));
            }
        }
        routes.push(RouteEntry {
            destination: candidate(extracted, selected.destination_idx, "destination")?.to_string(),
            gateway: candidate(extracted, selected.gateway_idx, "gateway")?.to_string(),
            interface: candidate(extracted, selected.interface_idx, "interface")?.to_string(),
            flags: selected
                .flags_idx
                .map(|index| candidate(extracted, index, "flags").map(ToOwned::to_owned))
                .transpose()?,
            metric: selected.metric,
        });
    }
    let table = UniversalRouteTable {
        version: "1.0".to_string(),
        metadata: RouteMetadata {
            generated_at: generated_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            source_device: device_name.to_string(),
            os_type: os_type.to_string(),
        },
        routes,
    };
    table
        .validate()
        .map_err(|error| format!("canonical route schema validation failed: {error}"))?;
    Ok(table)
}

pub fn evidence(extracted: &ExtractedRouteCandidates) -> RouteCanonicalizationEvidence {
    RouteCanonicalizationEvidence {
        candidates: extracted.candidates.clone(),
        lines: extracted.evidence.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconstructs_only_same_line_route_candidates() {
        let raw = "Destination Gateway Flags Netif Expire\ndefault 192.0.2.1 UGSc en0\n192.0.2.0/24 link#4 UCS en0";
        let extracted = extract(raw);
        assert_eq!(extracted.evidence.len(), 2);
        let table = reconstruct_and_validate(
            RouteSelection {
                entries: vec![
                    RouteEntrySelection {
                        line_idx: 0,
                        destination_idx: 0,
                        gateway_idx: 1,
                        interface_idx: 3,
                        flags_idx: Some(2),
                        metric: None,
                    },
                    RouteEntrySelection {
                        line_idx: 1,
                        destination_idx: 4,
                        gateway_idx: 5,
                        interface_idx: 3,
                        flags_idx: Some(6),
                        metric: None,
                    },
                ],
            },
            &extracted,
            "r1",
            "macos",
            Utc::now(),
        )
        .unwrap();
        assert_eq!(table.routes[0].gateway, "192.0.2.1");
        let error = reconstruct_and_validate(
            RouteSelection {
                entries: vec![
                    RouteEntrySelection {
                        line_idx: 0,
                        destination_idx: 0,
                        gateway_idx: 5,
                        interface_idx: 3,
                        flags_idx: Some(2),
                        metric: None,
                    },
                    RouteEntrySelection {
                        line_idx: 1,
                        destination_idx: 4,
                        gateway_idx: 5,
                        interface_idx: 3,
                        flags_idx: Some(6),
                        metric: None,
                    },
                ],
            },
            &extracted,
            "r1",
            "macos",
            Utc::now(),
        )
        .unwrap_err();
        assert!(error.contains("does not occur"));
    }

    #[test]
    fn canonicalizes_ipv6_destinations_gateways_and_zone_ids() {
        let raw = "Destination Gateway Flags Netif\n2001:db8:1::/64 fe80::1%en0 UG en0\n::/0 fe80::2%en0 UG en0";
        let extracted = extract(raw);
        assert_eq!(extracted.evidence.len(), 2);
        let table = reconstruct_and_validate(
            RouteSelection {
                entries: vec![
                    RouteEntrySelection {
                        line_idx: 0,
                        destination_idx: 0,
                        gateway_idx: 1,
                        interface_idx: 3,
                        flags_idx: Some(2),
                        metric: None,
                    },
                    RouteEntrySelection {
                        line_idx: 1,
                        destination_idx: 4,
                        gateway_idx: 5,
                        interface_idx: 3,
                        flags_idx: Some(2),
                        metric: None,
                    },
                ],
            },
            &extracted,
            "r1",
            "ios",
            Utc::now(),
        )
        .unwrap();
        assert_eq!(table.routes[0].destination, "2001:db8:1::/64");
        assert_eq!(table.routes[0].gateway, "fe80::1%en0");
        assert_eq!(table.routes[1].destination, "::/0");
    }

    #[test]
    fn accepts_a_valid_subset_when_other_route_lines_are_ambiguous() {
        let raw = "Destination Gateway Flags Netif\ndefault 192.0.2.1 UGSc en0\n192.0.2.0/24 link#4 UCS en0";
        let extracted = extract(raw);

        let table = reconstruct_and_validate(
            RouteSelection {
                entries: vec![RouteEntrySelection {
                    line_idx: 0,
                    destination_idx: 0,
                    gateway_idx: 1,
                    interface_idx: 3,
                    flags_idx: Some(2),
                    metric: None,
                }],
            },
            &extracted,
            "r1",
            "macos",
            Utc::now(),
        )
        .unwrap();

        assert_eq!(table.routes.len(), 1);
        assert_eq!(table.routes[0].destination, "default");
    }
}
