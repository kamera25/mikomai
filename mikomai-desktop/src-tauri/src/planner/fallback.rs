use crate::state::events::Decision;
use crate::state::network_state::NetworkState;

/// Fill an incomplete RAG action from the original goal and registered device.
/// A missing query would otherwise become an unscoped search over all vendors.
pub fn complete_rag_decision(
    decision: &mut Decision,
    goal: &str,
    connections: &[crate::connections::Connection],
) {
    if !matches!(
        decision.tool.as_deref(),
        Some("query_nw_db" | "network_query_nw_db" | "query_rag")
    ) || !matches!(
        decision.action_type,
        crate::state::events::ActionType::Observe | crate::state::events::ActionType::Verify
    ) {
        return;
    }

    let target_connection = decision
        .target
        .as_deref()
        .and_then(|target| connections.iter().find(|conn| conn.matches_host_or_ip(target)))
        .or_else(|| {
            let lower_goal = goal.to_lowercase();
            connections.iter().find(|conn| {
                let hostname = conn.hostname.as_str().to_lowercase();
                let ip = conn.ip_string();
                (!hostname.is_empty() && lower_goal.contains(&hostname))
                    || (!ip.is_empty() && lower_goal.contains(&ip.to_lowercase()))
            })
        });

    if decision.target.is_none() {
        if let Some(conn) = target_connection {
            decision.target = Some(conn.hostname.to_string());
        }
    }

    let query = decision
        .parameters
        .get("query")
        .and_then(serde_json::Value::as_str)
        .filter(|query| !query.trim().is_empty())
        .unwrap_or(goal)
        .trim();
    let query = if let Some(conn) = target_connection {
        if let Some(brand) = crate::mcp::rag::vendor::registered_connection_brand(conn) {
            if query.contains("[Context:") {
                query.to_string()
            } else {
                format!("[Context: {brand}] {query}")
            }
        } else {
            query.to_string()
        }
    } else {
        query.to_string()
    };

    if !decision.parameters.is_object() {
        decision.parameters = serde_json::json!({});
    }
    decision.parameters["query"] = serde_json::Value::String(query);
}

pub fn is_explanatory_request(goal: &str) -> bool {
    [
        "教えて",
        "方法",
        "手順",
        "設定例",
        "コマンド",
        "とは",
        "解説",
    ]
    .iter()
    .any(|marker| goal.contains(marker))
}

pub fn finish_with_evidence_answer(decision: &mut Decision, goal: &str, answer: String) {
    decision.action_type = crate::state::events::ActionType::Finish;
    decision.objective = goal.to_string();
    decision.tool = None;
    decision.target = None;
    decision.parameters = serde_json::Value::Null;
    decision.reason.clear();
    decision.expected_observation.clear();
    decision.final_answer = Some(answer);
}

/// Convert command-specification questions into a vendor-scoped RAG lookup.
/// This is a deterministic backstop for cases where the model ignores the
/// planner prompt and asks the user for a command that NW-DB may already have.
pub fn fallback_to_rag_for_known_vendor(
    decision: &mut Decision,
    device_vendors: &[(String, String)],
    goal: &str,
) {
    if decision.action_type != crate::state::events::ActionType::AskHuman {
        return;
    }

    let command_question =
        format!("{} {}", decision.objective, decision.reason.join(" ")).to_lowercase();
    let is_command_question = ["コマンド", "command", "構文", "仕様", "cli", "hostname"]
        .iter()
        .any(|term| command_question.contains(term));
    if !is_command_question {
        return;
    }

    let target = decision.target.as_deref().or_else(|| {
        device_vendors
            .iter()
            .find(|(hostname, _)| {
                let hostname = hostname.to_lowercase();
                command_question.contains(&hostname) || goal.to_lowercase().contains(&hostname)
            })
            .map(|(hostname, _)| hostname.as_str())
    });
    let Some(target) = target else { return };
    let Some((hostname, vendor)) = device_vendors
        .iter()
        .find(|(hostname, _)| hostname.eq_ignore_ascii_case(target))
    else {
        return;
    };

    let brand = crate::mcp::brands::get_brand(vendor).unwrap_or(vendor);
    decision.action_type = crate::state::events::ActionType::Observe;
    decision.objective = format!("{} ({}) のコマンド仕様をNW-DBで調査する", hostname, brand);
    decision.tool = Some("query_nw_db".to_string());
    decision.target = Some(hostname.clone());
    decision.parameters = serde_json::json!({
        "query": format!("[Context: {}] {} コマンド 設定", brand, goal),
    });
    decision.reason = vec![format!(
        "{} のベンダーは {} と判明しているため、ユーザーにコマンドを確認する前にNW-DBを検索する。",
        hostname, brand
    )];
    decision.expected_observation = vec!["対象機器で使える設定コマンドと手順".to_string()];
    decision.final_answer = None;
}

/// Returns source text when an explanatory RAG request has enough evidence,
/// but the Planner asks for parameters or finishes without an answer brief.
pub fn explanatory_rag_evidence<'a>(
    decision: &Decision,
    network_state: &'a NetworkState,
    goal: &str,
) -> Option<&'a str> {
    let needs_answer = decision.action_type == crate::state::events::ActionType::AskHuman
        || (decision.action_type == crate::state::events::ActionType::Finish
            && decision
                .final_answer
                .as_deref()
                .is_none_or(|answer| answer.trim().is_empty()));
    if !needs_answer
        || crate::harness::intent::is_configuration_change_request(goal)
        || !is_explanatory_request(goal)
    {
        return None;
    }

    let Some(evidence) = network_state
        .observed
        .observations
        .iter()
        .rev()
        .find(|observation| observation.source.tool_name.as_deref() == Some("rag_co_worker"))
        .map(|observation| observation.raw.trim())
    else {
        return None;
    };

    if evidence.is_empty()
        || evidence.contains("該当する情報が見つかりません")
        || evidence.contains("資料選定に失敗しました")
    {
        return None;
    }

    Some(evidence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::decision::parse_decision_from_json;

    #[test]
    fn rag_decision_recovers_f220_target_and_vendor_query() {
        let connections = vec![serde_json::from_value(serde_json::json!({
            "id": "1",
            "status": "offline",
            "hostname": "F220",
            "ip": null,
            "type": "Console",
            "lastConnected": "Never",
            "deviceType": "furukawa_fitelnet",
            "vendorType": null
        }))
        .unwrap()];
        let mut decision = parse_decision_from_json(
            r#"{"action_type":"OBSERVE","objective":"F220のVLAN設定に関するドキュメントやコマンド仕様を調査する","tool":"query_nw_db","target":null,"parameters":null}"#,
        )
        .unwrap();

        complete_rag_decision(
            &mut decision,
            "F220のVLAN設定方法を教えて",
            &connections,
        );

        assert_eq!(decision.target.as_deref(), Some("F220"));
        assert_eq!(
            decision.parameters["query"],
            "[Context: furukawa_fitelnet] F220のVLAN設定方法を教えて"
        );
        let action = crate::validator::schema::SchemaValidator::validate_decision(&decision)
            .unwrap();
        let args = crate::harness::execution::prepare_tool_arguments(
            &action,
            Some("F220のVLAN設定方法を教えて"),
        );
        assert_eq!(args["query"], decision.parameters["query"]);
        assert_eq!(args["target"], "F220");
    }

    #[test]
    fn rag_decision_preserves_specific_query_without_registered_target() {
        let mut decision = parse_decision_from_json(
            r#"{"action_type":"OBSERVE","objective":"VLANを調べる","tool":"query_nw_db","parameters":{"query":"[Context: Yamaha] VLAN 設定"}}"#,
        )
        .unwrap();
        complete_rag_decision(&mut decision, "VLANについて", &[]);
        assert_eq!(decision.target, None);
        assert_eq!(decision.parameters["query"], "[Context: Yamaha] VLAN 設定");
    }

    #[test]
    fn known_vendor_command_question_falls_back_to_rag() {
        let mut decision = parse_decision_from_json(
            r#"{
            "action_type": "ASK_HUMAN",
            "objective": "F220 の hostname 設定コマンドを確認する",
            "target": "F220",
            "reason": ["FITELnet のコマンド仕様が不明"]
        }"#,
        )
        .unwrap();

        fallback_to_rag_for_known_vendor(
            &mut decision,
            &[("F220".to_string(), "furukawa_fitelnet".to_string())],
            "F220 に hostname aaa を設定する",
        );

        assert_eq!(
            decision.action_type,
            crate::state::events::ActionType::Observe
        );
        assert_eq!(decision.tool.as_deref(), Some("query_nw_db"));
        assert_eq!(decision.target.as_deref(), Some("F220"));
        assert_eq!(
            decision
                .parameters
                .get("query")
                .and_then(|value| value.as_str()),
            Some("[Context: furukawa_fitelnet] F220 に hostname aaa を設定する コマンド 設定")
        );
    }

    #[test]
    fn non_command_question_remains_ask_human() {
        let mut decision = parse_decision_from_json(
            r#"{
            "action_type": "ASK_HUMAN",
            "objective": "変更実施の承認を確認する",
            "target": "F220",
            "reason": ["設定変更にはユーザー承認が必要"]
        }"#,
        )
        .unwrap();

        fallback_to_rag_for_known_vendor(
            &mut decision,
            &[("F220".to_string(), "furukawa_fitelnet".to_string())],
            "F220 に hostname aaa を設定する",
        );

        assert_eq!(
            decision.action_type,
            crate::state::events::ActionType::AskHuman
        );
    }

    #[test]
    fn explanatory_rag_request_finishes_without_requesting_parameters() {
        let mut state = NetworkState::with_goal("F220のTrunk VLANの設定を教えて".to_string());
        state.apply_observation(crate::state::events::Observation {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            raw: "=== 選択資料: F220 Trunk VLAN ===\ninterface GigaEthernet <INTERFACE>.<VLAN_ID>\n vlan-id <VLAN_ID>\n exit".to_string(),
            parsed: None,
            source: crate::state::events::ObservationSource {
                device: Some("F220".to_string()),
                command: None,
                tool_name: Some("rag_co_worker".to_string()),
                tool_kind: None,
                parameters: None,
            },
            provenance: crate::state::events::Provenance {
                origin: crate::state::events::ProvenanceOrigin::Llm,
                confidence: None,
            },
        });
        let mut decision = parse_decision_from_json(
            r#"{"action_type":"ASK_HUMAN","objective":"VLAN IDとポートを確認する","reason":["設定値が未指定"]}"#,
        )
        .unwrap();

        let evidence =
            explanatory_rag_evidence(&decision, &state, "F220のTrunk VLANの設定を教えて").unwrap();
        let mut missing_brief = parse_decision_from_json(
            r#"{"action_type":"FINISH","objective":"F220のVLAN設定を回答する","tool":null}"#,
        )
        .unwrap();
        assert_eq!(
            explanatory_rag_evidence(&missing_brief, &state, "F220のTrunk VLANの設定を教えて"),
            Some(evidence)
        );
        finish_with_evidence_answer(
            &mut missing_brief,
            "F220のTrunk VLANの設定を教えて",
            "資料から作成した完了メモ".to_string(),
        );
        assert!(explanatory_rag_evidence(
            &missing_brief,
            &state,
            "F220のTrunk VLANの設定を教えて"
        )
        .is_none());
        finish_with_evidence_answer(
            &mut decision,
            "F220のTrunk VLANの設定を教えて",
            format!("資料を要約した回答です。\n\n{evidence}"),
        );

        assert_eq!(
            decision.action_type,
            crate::state::events::ActionType::Finish
        );
        assert!(decision.final_answer.unwrap().contains("<VLAN_ID>"));
    }
}
