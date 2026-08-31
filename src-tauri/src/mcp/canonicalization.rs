//! Reusable building blocks for constrained CLI canonicalization.
//!
//! Extractors discover values deterministically.  A model may only return
//! indexes into those vectors plus explicitly enumerated semantic fields.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CandidateVectors {
    pub ip_addresses: Vec<String>,
    pub mac_addresses: Vec<String>,
    pub interfaces: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceLine {
    pub line: usize,
    pub text: String,
    pub ip_indexes: Vec<usize>,
    pub mac_indexes: Vec<usize>,
    pub interface_indexes: Vec<usize>,
    /// Decimal scalar tokens on the line (age, TTL, metric, etc.).
    pub scalar_values: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedCandidates {
    pub candidates: CandidateVectors,
    pub evidence: Vec<EvidenceLine>,
}

fn ip_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b(?:25[0-5]|2[0-4]\d|1?\d?\d)(?:\.(?:25[0-5]|2[0-4]\d|1?\d?\d)){3}\b").unwrap())
}

fn mac_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(?:[0-9a-f]{2}[:-]){5}[0-9a-f]{2}\b|\b[0-9a-f]{4}\.[0-9a-f]{4}\.[0-9a-f]{4}\b").unwrap())
}

pub fn normalize_mac(value: &str) -> String {
    value.chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect::<String>()
        .to_ascii_lowercase()
        .as_bytes()
        .chunks(2)
        .map(std::str::from_utf8)
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_default()
        .join(":")
}

/// Extract values and their source-line evidence. `interface_from_line` lets
/// other protocol adapters share the candidate/evidence machinery.
pub fn extract_candidates<F>(raw: &str, interface_from_line: F) -> ExtractedCandidates
where
    F: Fn(&str) -> Option<String>,
{
    let mut candidates = CandidateVectors::default();
    let mut evidence = Vec::new();
    for (line_number, text) in raw.lines().enumerate() {
        let ips = ip_re().find_iter(text).map(|m| m.as_str().to_string()).collect::<Vec<_>>();
        let macs = mac_re().find_iter(text).map(|m| normalize_mac(m.as_str())).collect::<Vec<_>>();
        let interface = interface_from_line(text);
        // Numeric scalar attributes are complete whitespace-delimited tokens.
        // This intentionally excludes digits embedded in addresses (IPs, MACs,
        // and interface names such as LAN1 or Gi1/0/1).
        let scalar_values = text.split_whitespace()
            .filter_map(|token| token.trim_matches(|c: char| matches!(c, '(' | ')' | ',' | ';' | '[' | ']')).parse::<u32>().ok())
            .collect();
        let ip_indexes: Vec<usize> = ips.iter().map(|value| intern(&mut candidates.ip_addresses, value)).collect();
        let mac_indexes: Vec<usize> = macs.iter().map(|value| intern(&mut candidates.mac_addresses, value)).collect();
        let interface_indexes: Vec<usize> = interface.iter().map(|value| intern(&mut candidates.interfaces, value)).collect();
        if !ip_indexes.is_empty() || !mac_indexes.is_empty() || !interface_indexes.is_empty() {
            evidence.push(EvidenceLine { line: line_number + 1, text: text.to_string(), ip_indexes, mac_indexes, interface_indexes, scalar_values });
        }
    }
    ExtractedCandidates { candidates, evidence }
}

fn intern(values: &mut Vec<String>, value: &str) -> usize {
    match values.iter().position(|existing| existing == value) {
        Some(index) => index,
        None => { values.push(value.to_string()); values.len() - 1 }
    }
}

pub fn ensure_unique<T: std::hash::Hash + Eq>(values: impl IntoIterator<Item = T>, label: &str) -> Result<(), String> {
    let mut seen = HashSet::new();
    for value in values {
        if !seen.insert(value) { return Err(format!("duplicate {label} selected")); }
    }
    Ok(())
}
