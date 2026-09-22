use crate::state::events::Decision;
use crate::state::network_state::NetworkState;

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

/// Returns source text when a documentation request was incorrectly routed
/// into a parameter interview after RAG had already found relevant material.
pub fn explanatory_rag_evidence<'a>(
    decision: &Decision,
    network_state: &'a NetworkState,
    goal: &str,
) -> Option<&'a str> {
    if decision.action_type != crate::state::events::ActionType::AskHuman
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
            raw: "=== 選択資料: Trunk VLAN ===\ninterface <INTERFACE>\nswitchport trunk allowed vlan <VLAN_ID>".to_string(),
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
