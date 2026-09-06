//! Constrained packet inspection and preview generation.
//!
//! This module intentionally does *not* capture or transmit packets. It is
//! safe to call from the unattended MCP path because it only parses caller
//! supplied frame bytes or creates an in-memory DHCPREQUEST preview. A future
//! transmitter must be an OS-specific privileged helper invoked exclusively by
//! a hash-bound, user-approved operation plan.

use crate::network::CommandResult;
use etherparse::{PacketBuilder, SlicedPacket};
use ring::digest::{digest, SHA256};
use serde::Serialize;
use std::net::Ipv4Addr;

const MAX_FRAME_BYTES: usize = 1536;
const DHCP_MIN_BOOTP_BYTES: usize = 300;

#[derive(Debug, Clone, Default)]
pub struct DhcpRequestPreviewInput {
    pub client_mac: Option<String>,
    pub transaction_id: Option<String>,
    pub requested_ip: Option<String>,
    pub server_identifier: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FrameAnalysis {
    valid_ethernet: bool,
    bytes: usize,
    broadcast_destination: bool,
    vlan_tagged: bool,
    dhcp_client_to_server: bool,
    verdict: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DhcpPreview {
    packet_type: &'static str,
    mode: &'static str,
    destination: &'static str,
    source_port: u16,
    destination_port: u16,
    bytes: usize,
    sha256: String,
    safety_note: &'static str,
}

/// Parses an Ethernet-II frame supplied as hexadecimal. No frame payload or
/// identifiers are written to output or audit logs.
pub fn analyze_ethernet_frame_hex(frame_hex: &str) -> Result<CommandResult, String> {
    let frame = decode_hex_frame(frame_hex)?;
    let packet = SlicedPacket::from_ethernet(&frame)
        .map_err(|error| format!("Invalid Ethernet frame: {error}"))?;
    let vlan_tagged = !packet.link_exts.is_empty();
    let broadcast_destination = frame[..6].iter().all(|byte| *byte == 0xff);
    let dhcp_client_to_server = is_dhcp_client_to_server(&frame, vlan_tagged);
    let verdict = if broadcast_destination && dhcp_client_to_server {
        "DHCP client broadcast observed"
    } else if broadcast_destination {
        "Broadcast frame observed"
    } else {
        "Unicast or multicast frame observed"
    };
    json_result(FrameAnalysis {
        valid_ethernet: true,
        bytes: frame.len(),
        broadcast_destination,
        vlan_tagged,
        dhcp_client_to_server,
        verdict,
    })
}

/// Creates one standards-shaped DHCPREQUEST frame in memory for review. The
/// bytes are never returned, persisted, captured, or transmitted.
pub fn prepare_dhcp_request_preview(
    input: DhcpRequestPreviewInput,
) -> Result<CommandResult, String> {
    let client_mac = parse_mac(required(input.client_mac, "client_mac")?)?;
    let transaction_id = parse_xid(required(input.transaction_id, "transaction_id")?)?;
    let requested_ip: Ipv4Addr = required(input.requested_ip, "requested_ip")?
        .parse()
        .map_err(|_| "requested_ip must be a valid IPv4 address")?;
    let server_identifier: Ipv4Addr = required(input.server_identifier, "server_identifier")?
        .parse()
        .map_err(|_| "server_identifier must be a valid IPv4 address")?;

    let payload = dhcp_request_payload(client_mac, transaction_id, requested_ip, server_identifier);
    let builder = PacketBuilder::ethernet2(client_mac, [0xff; 6])
        .ipv4([0, 0, 0, 0], [255, 255, 255, 255], 64)
        .udp(68, 67);
    let mut frame = Vec::with_capacity(builder.size(payload.len()));
    builder
        .write(&mut frame, &payload)
        .map_err(|error| format!("Failed to build DHCPREQUEST preview: {error}"))?;

    // Verify the builder output before reporting success. This detects API or
    // serialization changes without ever putting a frame on an interface.
    SlicedPacket::from_ethernet(&frame)
        .map_err(|error| format!("Generated preview did not parse: {error}"))?;
    let sha256 = digest(&SHA256, &frame)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    json_result(DhcpPreview {
        packet_type: "DHCPREQUEST",
        mode: "preview_only",
        destination: "255.255.255.255:67",
        source_port: 68,
        destination_port: 67,
        bytes: frame.len(),
        sha256,
        safety_note: "No packet was transmitted. Sending DHCP traffic requires a separately approved, allowlisted operation plan.",
    })
}

fn required(value: Option<String>, name: &str) -> Result<String, String> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name} is required before a DHCPREQUEST preview can be generated"))
}

fn decode_hex_frame(input: &str) -> Result<Vec<u8>, String> {
    let compact: String = input
        .chars()
        .filter(|char| !char.is_ascii_whitespace() && *char != ':')
        .collect();
    if compact.len() % 2 != 0 {
        return Err("frame_hex must contain complete hexadecimal byte pairs".into());
    }
    if compact.len() / 2 > MAX_FRAME_BYTES {
        return Err(format!(
            "frame_hex exceeds the {MAX_FRAME_BYTES}-byte safety limit"
        ));
    }
    if compact.len() < 28 {
        return Err("frame_hex is shorter than an Ethernet-II header".into());
    }
    (0..compact.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&compact[index..index + 2], 16)
                .map_err(|_| "frame_hex must contain only hexadecimal characters".to_string())
        })
        .collect()
}

fn parse_mac(value: String) -> Result<[u8; 6], String> {
    let compact: String = value
        .chars()
        .filter(|char| char.is_ascii_hexdigit())
        .collect();
    if compact.len() != 12 {
        return Err("client_mac must be a six-octet MAC address".into());
    }
    let bytes: Vec<u8> = (0..12)
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&compact[index..index + 2], 16)
                .map_err(|_| "client_mac must be hexadecimal".to_string())
        })
        .collect::<Result<_, _>>()?;
    bytes
        .try_into()
        .map_err(|_| "client_mac must be a six-octet MAC address".into())
}

fn parse_xid(value: String) -> Result<[u8; 4], String> {
    let value = value.trim().trim_start_matches("0x");
    if value.len() != 8 || !value.chars().all(|char| char.is_ascii_hexdigit()) {
        return Err("transaction_id must be an 8-digit hexadecimal value".into());
    }
    let bytes: Vec<u8> = (0..8)
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16)
                .map_err(|_| "transaction_id must be hexadecimal".to_string())
        })
        .collect::<Result<_, _>>()?;
    bytes
        .try_into()
        .map_err(|_| "transaction_id must be four octets".into())
}

fn dhcp_request_payload(
    client_mac: [u8; 6],
    transaction_id: [u8; 4],
    requested_ip: Ipv4Addr,
    server_identifier: Ipv4Addr,
) -> Vec<u8> {
    let mut payload = vec![0_u8; DHCP_MIN_BOOTP_BYTES];
    payload[0] = 1; // BOOTREQUEST
    payload[1] = 1; // Ethernet
    payload[2] = 6; // MAC length
    payload[4..8].copy_from_slice(&transaction_id);
    payload[10..12].copy_from_slice(&0x8000_u16.to_be_bytes()); // broadcast reply requested
    payload[28..34].copy_from_slice(&client_mac);
    payload[236..240].copy_from_slice(&[99, 130, 83, 99]); // DHCP magic cookie
    let requested = requested_ip.octets();
    let server = server_identifier.octets();
    let options = [
        53,
        1,
        3, // DHCPREQUEST
        50,
        4,
        requested[0],
        requested[1],
        requested[2],
        requested[3],
        54,
        4,
        server[0],
        server[1],
        server[2],
        server[3],
        55,
        3,
        1,
        3,
        6, // subnet mask, router, DNS
        255,
    ];
    payload[240..240 + options.len()].copy_from_slice(&options);
    payload
}

fn is_dhcp_client_to_server(frame: &[u8], vlan_tagged: bool) -> bool {
    let ip_offset = if vlan_tagged { 18 } else { 14 };
    if frame.len() < ip_offset + 28 || frame[ip_offset] >> 4 != 4 || frame[ip_offset + 9] != 17 {
        return false;
    }
    let ihl = usize::from(frame[ip_offset] & 0x0f) * 4;
    let udp_offset = ip_offset + ihl;
    frame.len() >= udp_offset + 4
        && u16::from_be_bytes([frame[udp_offset], frame[udp_offset + 1]]) == 68
        && u16::from_be_bytes([frame[udp_offset + 2], frame[udp_offset + 3]]) == 67
}

fn json_result<T: Serialize>(value: T) -> Result<CommandResult, String> {
    Ok(CommandResult {
        success: true,
        output: serde_json::to_string_pretty(&value)
            .map_err(|error| format!("Failed to serialize packet diagnostic: {error}"))?,
        saved_path: None,
        is_cached: None,
        cache_time: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_requires_every_dhcp_identity_field() {
        let error = prepare_dhcp_request_preview(DhcpRequestPreviewInput::default()).unwrap_err();
        assert!(error.contains("client_mac"));
    }

    #[test]
    fn preview_is_parseable_but_never_transmitted() {
        let result = prepare_dhcp_request_preview(DhcpRequestPreviewInput {
            client_mac: Some("02:00:00:00:00:01".into()),
            transaction_id: Some("1234abcd".into()),
            requested_ip: Some("192.0.2.20".into()),
            server_identifier: Some("192.0.2.1".into()),
        })
        .unwrap();
        assert!(result.output.contains("preview_only"));
        assert!(result.output.contains("No packet was transmitted"));
        assert!(!result.output.contains("02:00:00:00:00:01"));
    }

    #[test]
    fn broadcast_dhcp_frame_is_classified_without_exposing_payload() {
        let payload = dhcp_request_payload(
            [2, 0, 0, 0, 0, 1],
            [1, 2, 3, 4],
            "192.0.2.20".parse().unwrap(),
            "192.0.2.1".parse().unwrap(),
        );
        let builder = PacketBuilder::ethernet2([2, 0, 0, 0, 0, 1], [0xff; 6])
            .ipv4([0, 0, 0, 0], [255, 255, 255, 255], 64)
            .udp(68, 67);
        let mut frame = Vec::new();
        builder.write(&mut frame, &payload).unwrap();
        let frame_hex: String = frame.iter().map(|byte| format!("{byte:02x}")).collect();
        let result = analyze_ethernet_frame_hex(&frame_hex).unwrap();
        assert!(result.output.contains("DHCP client broadcast observed"));
        assert!(!result.output.contains(&frame_hex));
    }

    #[test]
    fn invalid_or_oversized_frame_is_rejected() {
        assert!(analyze_ethernet_frame_hex("not hex").is_err());
        assert!(analyze_ethernet_frame_hex(&"aa".repeat(MAX_FRAME_BYTES + 1)).is_err());
    }
}
