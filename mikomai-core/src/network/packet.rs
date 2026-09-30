//! Portable, read-only parsing of Ethernet frame samples supplied by the user.
//! This module never captures or transmits packets.

use etherparse::PacketBuilder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::net::Ipv4Addr;

const MAX_FRAME_BYTES: usize = 1536;
const DHCP_MIN_BOOTP_BYTES: usize = 300;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DhcpRequestPreviewInput {
    pub client_mac: Option<String>,
    pub transaction_id: Option<String>,
    pub requested_ip: Option<String>,
    pub server_identifier: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PacketSafetyIntent {
    AnalyzeBroadcast,
    AnalyzeDhcpResponse,
    PrepareDhcpRequest,
    DhcpRequestProbe,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PacketSafetyRequest {
    pub intent: Option<PacketSafetyIntent>,
    pub frame_hex: Option<String>,
    pub client_mac: Option<String>,
    pub transaction_id: Option<String>,
    pub requested_ip: Option<String>,
    pub server_identifier: Option<String>,
    pub interface: Option<String>,
    pub vlan: Option<u16>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PacketSafetyOutcome {
    AwaitingUserInput { message: String },
    Completed { completion_brief: String },
    AwaitingApproval { message: String },
    Failed { public_message: String },
}

/// Runs the old packet-safety worker's portable, non-transmitting behavior.
/// A probe can only request approval; this function never opens a socket.
pub fn run_packet_safety(request: PacketSafetyRequest) -> PacketSafetyOutcome {
    let Some(intent) = request.intent else {
        return PacketSafetyOutcome::AwaitingUserInput {
            message: "診断種別を指定してください: analyze_broadcast, analyze_dhcp_response, prepare_dhcp_request, dhcp_request_probe".into(),
        };
    };
    match intent {
        PacketSafetyIntent::AnalyzeBroadcast | PacketSafetyIntent::AnalyzeDhcpResponse => {
            let Some(frame) = request.frame_hex.filter(|frame| !frame.trim().is_empty()) else {
                return PacketSafetyOutcome::AwaitingUserInput {
                    message: "解析する frame_hex を指定してください。最大1,536バイトのEthernet IIフレームだけを受け付けます。".into(),
                };
            };
            match analyze_ethernet_frame_hex(&frame) {
                Ok(result) => PacketSafetyOutcome::Completed {
                    completion_brief: result,
                },
                Err(error) => PacketSafetyOutcome::Failed {
                    public_message: format!("パケット解析を実行できませんでした: {error}"),
                },
            }
        }
        PacketSafetyIntent::PrepareDhcpRequest | PacketSafetyIntent::DhcpRequestProbe => {
            let mut missing = missing_dhcp_fields(&request);
            if intent == PacketSafetyIntent::DhcpRequestProbe {
                if request
                    .interface
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
                {
                    missing.push("interface");
                }
                if request.vlan.is_none() {
                    missing.push("vlan");
                }
            }
            if !missing.is_empty() {
                return PacketSafetyOutcome::AwaitingUserInput {
                    message: format!("DHCP要求に必要な値が不足しています: {}", missing.join(", ")),
                };
            }
            if intent == PacketSafetyIntent::DhcpRequestProbe {
                if let Err(error) = validate_probe_scope(&request) {
                    return PacketSafetyOutcome::Failed {
                        public_message: error,
                    };
                }
            }
            let input = DhcpRequestPreviewInput {
                client_mac: request.client_mac.clone(),
                transaction_id: request.transaction_id.clone(),
                requested_ip: request.requested_ip.clone(),
                server_identifier: request.server_identifier.clone(),
            };
            match prepare_dhcp_request_preview(input) {
                Err(error) => PacketSafetyOutcome::Failed { public_message: format!("DHCPREQUESTプレビューを生成できませんでした: {error}") },
                Ok(_) if intent == PacketSafetyIntent::DhcpRequestProbe => PacketSafetyOutcome::AwaitingApproval {
                    message: "DHCPREQUESTの実送信には、対象インターフェースの許可リスト、ハッシュ固定の実行計画、明示承認、単発送信ヘルパーが必要です。このWorkerは送信しません。".into(),
                },
                Ok(preview) => PacketSafetyOutcome::Completed { completion_brief: preview },
            }
        }
    }
}

fn missing_dhcp_fields(request: &PacketSafetyRequest) -> Vec<&'static str> {
    [
        ("client_mac", request.client_mac.as_ref()),
        ("transaction_id", request.transaction_id.as_ref()),
        ("requested_ip", request.requested_ip.as_ref()),
        ("server_identifier", request.server_identifier.as_ref()),
    ]
    .into_iter()
    .filter_map(|(name, value)| {
        value
            .is_none_or(|value| value.trim().is_empty())
            .then_some(name)
    })
    .collect()
}

fn validate_probe_scope(request: &PacketSafetyRequest) -> Result<(), String> {
    let interface = request.interface.as_deref().unwrap_or_default();
    if interface.len() > 64
        || interface.eq_ignore_ascii_case("lo")
        || !interface
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
    {
        return Err("interface is not a permitted interface identifier".into());
    }
    match request.vlan {
        Some(1..=4094) => Ok(()),
        _ => Err("vlan must be between 1 and 4094".into()),
    }
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

/// Builds and validates a DHCPREQUEST preview in memory. It never sends packets.
pub fn prepare_dhcp_request_preview(input: DhcpRequestPreviewInput) -> Result<String, String> {
    let mac = parse_mac(input.client_mac.ok_or("client_mac is required")?)?;
    let xid = parse_xid(input.transaction_id.ok_or("transaction_id is required")?)?;
    let requested: Ipv4Addr = input
        .requested_ip
        .ok_or("requested_ip is required")?
        .parse()
        .map_err(|_| "requested_ip must be a valid IPv4 address")?;
    let server: Ipv4Addr = input
        .server_identifier
        .ok_or("server_identifier is required")?
        .parse()
        .map_err(|_| "server_identifier must be a valid IPv4 address")?;
    let payload = dhcp_request_payload(mac, xid, requested, server);
    let builder = PacketBuilder::ethernet2(mac, [0xff; 6])
        .ipv4([0, 0, 0, 0], [255, 255, 255, 255], 64)
        .udp(68, 67);
    let mut frame = Vec::with_capacity(builder.size(payload.len()));
    builder
        .write(&mut frame, &payload)
        .map_err(|error| error.to_string())?;
    let hash = Sha256::digest(&frame)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let preview = DhcpPreview {
        packet_type: "DHCPREQUEST",
        mode: "preview_only",
        destination: "255.255.255.255:67",
        source_port: 68,
        destination_port: 67,
        bytes: frame.len(),
        sha256: hash,
        safety_note:
            "No packet was transmitted. Transmission needs an explicitly approved operation.",
    };
    serde_json::to_string(&preview).map_err(|error| error.to_string())
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EthernetFrameSummary {
    pub bytes: usize,
    pub broadcast_destination: bool,
    pub vlan_tagged: bool,
    pub ether_type: String,
    pub dhcp_client_to_server: bool,
    pub verdict: String,
}

pub fn analyze_ethernet_frame_hex(input: &str) -> Result<String, String> {
    let compact: String = input
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace() && *ch != ':')
        .collect();
    if compact.len() % 2 != 0 {
        return Err("frame_hex must contain complete hexadecimal byte pairs".into());
    }
    if compact.len() / 2 > MAX_FRAME_BYTES {
        return Err(format!("frame_hex exceeds {MAX_FRAME_BYTES} bytes"));
    }
    if compact.len() < 28 {
        return Err("frame_hex is shorter than an Ethernet-II frame".into());
    }
    let frame = (0..compact.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&compact[i..i + 2], 16)
                .map_err(|_| "frame_hex must contain hexadecimal characters".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let broadcast_destination = frame[..6].iter().all(|byte| *byte == 0xff);
    let mut offset = 14;
    let mut ether_type = u16::from_be_bytes([frame[12], frame[13]]);
    let mut vlan_tagged = false;
    while matches!(ether_type, 0x8100 | 0x88a8 | 0x9100) {
        vlan_tagged = true;
        if frame.len() < offset + 4 {
            return Err("truncated VLAN-tagged Ethernet frame".into());
        }
        ether_type = u16::from_be_bytes([frame[offset + 2], frame[offset + 3]]);
        offset += 4;
    }
    let dhcp_client_to_server = if ether_type == 0x0800 && frame.len() >= offset + 20 {
        let ihl = ((frame[offset] & 0x0f) as usize) * 4;
        if ihl < 20 || frame.len() < offset + ihl + 8 || frame[offset + 9] != 17 {
            false
        } else {
            let fragment = u16::from_be_bytes([frame[offset + 6], frame[offset + 7]]);
            let udp = offset + ihl;
            let src = u16::from_be_bytes([frame[udp], frame[udp + 1]]);
            let dst = u16::from_be_bytes([frame[udp + 2], frame[udp + 3]]);
            fragment & 0x1fff == 0 && src == 68 && dst == 67
        }
    } else {
        false
    };
    let verdict = if broadcast_destination && dhcp_client_to_server {
        "DHCP client broadcast"
    } else if broadcast_destination {
        "Ethernet broadcast"
    } else {
        "Ethernet unicast or multicast"
    };
    let summary = EthernetFrameSummary {
        bytes: frame.len(),
        broadcast_destination,
        vlan_tagged,
        ether_type: format!("0x{ether_type:04x}"),
        dhcp_client_to_server,
        verdict: verdict.to_owned(),
    };
    serde_json::to_string(&summary).map_err(|error| error.to_string())
}

fn parse_mac(value: String) -> Result<[u8; 6], String> {
    let compact: String = value.chars().filter(|ch| ch.is_ascii_hexdigit()).collect();
    if compact.len() != 12 {
        return Err("client_mac must be a six-octet MAC address".into());
    }
    (0..12)
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&compact[index..index + 2], 16)
                .map_err(|_| "client_mac must be hexadecimal".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "client_mac must be a six-octet MAC address".into())
}

fn parse_xid(value: String) -> Result<[u8; 4], String> {
    let value = value.trim().trim_start_matches("0x");
    if value.len() != 8 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("transaction_id must be an 8-digit hexadecimal value".into());
    }
    (0..8)
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16)
                .map_err(|_| "transaction_id must be hexadecimal".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "transaction_id must be four octets".into())
}

fn dhcp_request_payload(
    mac: [u8; 6],
    xid: [u8; 4],
    requested: Ipv4Addr,
    server: Ipv4Addr,
) -> Vec<u8> {
    let mut payload = vec![0_u8; DHCP_MIN_BOOTP_BYTES];
    payload[0] = 1;
    payload[1] = 1;
    payload[2] = 6;
    payload[4..8].copy_from_slice(&xid);
    payload[10..12].copy_from_slice(&0x8000_u16.to_be_bytes());
    payload[28..34].copy_from_slice(&mac);
    payload[236..240].copy_from_slice(&[99, 130, 83, 99]);
    let requested = requested.octets();
    let server = server.octets();
    let options = [
        53,
        1,
        3,
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
        6,
        255,
    ];
    payload[240..240 + options.len()].copy_from_slice(&options);
    payload
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_broadcast_vlan_and_dhcp_without_network_io() {
        let frame = "ffffffffffff0011223344558100006408004500001c0000000040110000c0000201ffffffff0044004300080000";
        let result: EthernetFrameSummary =
            serde_json::from_str(&analyze_ethernet_frame_hex(frame).unwrap()).unwrap();
        assert!(result.broadcast_destination);
        assert!(result.vlan_tagged);
        assert!(result.dhcp_client_to_server);
    }

    #[test]
    fn rejects_truncated_and_oversized_frames() {
        assert!(analyze_ethernet_frame_hex("001122334455").is_err());
        assert!(analyze_ethernet_frame_hex(&"00".repeat(MAX_FRAME_BYTES + 1)).is_err());
    }

    #[test]
    fn dhcp_request_preview_is_valid_and_never_exposes_identity_or_transmits() {
        let preview = prepare_dhcp_request_preview(DhcpRequestPreviewInput {
            client_mac: Some("02:00:00:00:00:01".into()),
            transaction_id: Some("1234abcd".into()),
            requested_ip: Some("192.0.2.20".into()),
            server_identifier: Some("192.0.2.1".into()),
        })
        .unwrap();
        assert!(preview.contains("preview_only"));
        assert!(preview.contains("No packet was transmitted"));
        assert!(!preview.contains("02:00:00:00:00:01"));
    }
}
