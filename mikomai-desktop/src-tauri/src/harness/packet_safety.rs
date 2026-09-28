//! Deterministic, non-transmitting packet diagnostics worker.
//!
//! It is safe to invoke from an Agent because it accepts a fixed intent set,
//! validates every input, and returns the common `WorkerOutcome` contract.
//! Raw transmission is intentionally outside this worker until an approved,
//! allowlisted OS helper exists.

use crate::harness::coordinator::WorkerOutcome;
use crate::mcp::packet::{
    analyze_ethernet_frame_hex, prepare_dhcp_request_preview, DhcpRequestPreviewInput,
};
use crate::network::CommandResult;
use serde::Deserialize;

const MAX_INTERFACE_NAME_LEN: usize = 64;

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

pub struct PacketSafetyWorker;

impl PacketSafetyWorker {
    pub fn run(request: PacketSafetyRequest) -> WorkerOutcome {
        let Some(intent) = request.intent else {
            return WorkerOutcome::AwaitingUserInput {
                message: "診断種別を指定してください: analyze_broadcast, analyze_dhcp_response, prepare_dhcp_request, dhcp_request_probe".into(),
            };
        };

        match intent {
            PacketSafetyIntent::AnalyzeBroadcast | PacketSafetyIntent::AnalyzeDhcpResponse => {
                let Some(frame_hex) = non_empty(request.frame_hex) else {
                    return WorkerOutcome::AwaitingUserInput {
                        message: "解析する frame_hex を指定してください。最大1,536バイトのEthernet IIフレームだけを受け付けます。".into(),
                    };
                };
                match analyze_ethernet_frame_hex(&frame_hex) {
                    Ok(result) => WorkerOutcome::Completed {
                        completion_brief: result.output,
                    },
                    Err(error) => WorkerOutcome::Failed {
                        public_message: format!("パケット解析を実行できませんでした: {error}"),
                    },
                }
            }
            PacketSafetyIntent::PrepareDhcpRequest => {
                let missing = missing_dhcp_fields(&request);
                if !missing.is_empty() {
                    return WorkerOutcome::AwaitingUserInput {
                        message: format!(
                            "DHCPREQUESTプレビューに必要な値が不足しています: {}",
                            missing.join(", ")
                        ),
                    };
                }
                match prepare_dhcp_request_preview(dhcp_preview_input(&request)) {
                    Ok(result) => WorkerOutcome::Completed {
                        completion_brief: result.output,
                    },
                    Err(error) => WorkerOutcome::Failed {
                        public_message: format!(
                            "DHCPREQUESTプレビューを生成できませんでした: {error}"
                        ),
                    },
                }
            }
            PacketSafetyIntent::DhcpRequestProbe => {
                let mut missing = missing_dhcp_fields(&request);
                if non_empty(request.interface.clone()).is_none() {
                    missing.push("interface");
                }
                if request.vlan.is_none() {
                    missing.push("vlan");
                }
                if !missing.is_empty() {
                    return WorkerOutcome::AwaitingUserInput {
                        message: format!(
                            "DHCP応答確認に必要な値が不足しています: {}",
                            missing.join(", ")
                        ),
                    };
                }
                if let Err(error) = validate_probe_scope(&request) {
                    return WorkerOutcome::Failed {
                        public_message: error,
                    };
                }
                // A preview is created before approval so malformed DHCP data
                // cannot reach a future transmitter. Its raw bytes stay local.
                if let Err(error) = prepare_dhcp_request_preview(dhcp_preview_input(&request)) {
                    return WorkerOutcome::Failed {
                        public_message: format!(
                            "DHCPREQUESTプレビューを生成できませんでした: {error}"
                        ),
                    };
                }
                WorkerOutcome::AwaitingApproval {
                    message: "DHCPREQUESTの実送信には、対象インターフェースの許可リスト、ハッシュ固定の実行計画、明示承認、単発送信ヘルパーが必要です。このWorkerは送信しません。".into(),
                }
            }
        }
    }

    /// Adapter for the existing MCP registry. The JSON is intentionally a
    /// common WorkerOutcome, so an Agent can reason about input/approval
    /// states without interpreting worker-specific free-form text.
    pub fn execute(request: PacketSafetyRequest) -> Result<CommandResult, String> {
        let outcome = Self::run(request);
        let success = !matches!(outcome, WorkerOutcome::Failed { .. });
        let output = serde_json::to_string_pretty(&outcome)
            .map_err(|error| format!("Failed to serialize packet worker outcome: {error}"))?;
        Ok(CommandResult {
            success,
            output,
            saved_path: None,
            is_cached: None,
            cache_time: None,
        })
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn missing_dhcp_fields(request: &PacketSafetyRequest) -> Vec<&'static str> {
    let fields = [
        ("client_mac", request.client_mac.as_ref()),
        ("transaction_id", request.transaction_id.as_ref()),
        ("requested_ip", request.requested_ip.as_ref()),
        ("server_identifier", request.server_identifier.as_ref()),
    ];
    fields
        .into_iter()
        .filter_map(|(name, value)| {
            value
                .is_none_or(|value| value.trim().is_empty())
                .then_some(name)
        })
        .collect()
}

fn dhcp_preview_input(request: &PacketSafetyRequest) -> DhcpRequestPreviewInput {
    DhcpRequestPreviewInput {
        client_mac: request.client_mac.clone(),
        transaction_id: request.transaction_id.clone(),
        requested_ip: request.requested_ip.clone(),
        server_identifier: request.server_identifier.clone(),
    }
}

fn validate_probe_scope(request: &PacketSafetyRequest) -> Result<(), String> {
    let interface = request.interface.as_deref().unwrap_or_default();
    if interface.len() > MAX_INTERFACE_NAME_LEN
        || interface.eq_ignore_ascii_case("lo")
        || !interface.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':' | '/')
        })
    {
        return Err("interface is not a permitted interface identifier".into());
    }
    match request.vlan {
        Some(1..=4094) => Ok(()),
        _ => Err("vlan must be between 1 and 4094".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dhcp_request(intent: PacketSafetyIntent) -> PacketSafetyRequest {
        PacketSafetyRequest {
            intent: Some(intent),
            client_mac: Some("02:00:00:00:00:01".into()),
            transaction_id: Some("1234abcd".into()),
            requested_ip: Some("192.0.2.20".into()),
            server_identifier: Some("192.0.2.1".into()),
            interface: Some("en0".into()),
            vlan: Some(100),
            ..Default::default()
        }
    }

    #[test]
    fn worker_collects_only_missing_dhcp_fields() {
        let outcome = PacketSafetyWorker::run(PacketSafetyRequest {
            intent: Some(PacketSafetyIntent::PrepareDhcpRequest),
            ..Default::default()
        });
        assert!(
            matches!(outcome, WorkerOutcome::AwaitingUserInput { ref message }
            if message.contains("client_mac") && message.contains("server_identifier"))
        );
    }

    #[test]
    fn worker_previews_dhcp_without_exposing_the_frame() {
        let outcome = PacketSafetyWorker::run(dhcp_request(PacketSafetyIntent::PrepareDhcpRequest));
        assert!(
            matches!(outcome, WorkerOutcome::Completed { ref completion_brief }
            if completion_brief.contains("preview_only") && !completion_brief.contains("02:00:00:00:00:01"))
        );
    }

    #[test]
    fn probe_requires_approval_and_never_transmits() {
        let outcome = PacketSafetyWorker::run(dhcp_request(PacketSafetyIntent::DhcpRequestProbe));
        assert!(
            matches!(outcome, WorkerOutcome::AwaitingApproval { ref message }
            if message.contains("送信しません"))
        );
    }

    #[test]
    fn probe_rejects_out_of_scope_interface_or_vlan() {
        let mut request = dhcp_request(PacketSafetyIntent::DhcpRequestProbe);
        request.interface = Some("lo".into());
        assert!(matches!(
            PacketSafetyWorker::run(request),
            WorkerOutcome::Failed { .. }
        ));
        let mut request = dhcp_request(PacketSafetyIntent::DhcpRequestProbe);
        request.vlan = Some(4095);
        assert!(matches!(
            PacketSafetyWorker::run(request),
            WorkerOutcome::Failed { .. }
        ));
    }

    #[test]
    fn registry_adapter_preserves_the_common_status() {
        let result = PacketSafetyWorker::execute(PacketSafetyRequest::default()).unwrap();
        assert!(result.success);
        assert!(result.output.contains("awaiting_user_input"));
    }
}
