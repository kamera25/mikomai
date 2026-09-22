use crate::planner::fallback::finish_with_evidence_answer;
use crate::state::events::Decision;
use crate::state::network_state::NetworkState;

fn mac_in_goal(goal: &str) -> Option<String> {
    let pattern = regex::Regex::new(
        r"(?i)(?:[0-9a-f]{1,2}[:-]){5}[0-9a-f]{1,2}|(?:[0-9a-f]{4}\.){2}[0-9a-f]{4}",
    )
    .expect("valid MAC pattern");
    let normalized = pattern.find_iter(goal).find_map(|found| {
        let before = goal[..found.start()].chars().last();
        let after = goal[found.end()..].chars().next();
        if before.is_some_and(|c| c.is_ascii_hexdigit() || matches!(c, ':' | '-' | '.'))
            || after.is_some_and(|c| c.is_ascii_hexdigit() || matches!(c, ':' | '-' | '.'))
        {
            return None;
        }
        let value = found.as_str();
        if value.contains('.') {
            let digits = value.replace('.', "").to_ascii_lowercase();
            return Some(
                (0..6)
                    .map(|i| &digits[i * 2..i * 2 + 2])
                    .collect::<Vec<_>>()
                    .join(":"),
            );
        }
        Some(
            value
                .split([':', '-'])
                .map(|octet| format!("{octet:0>2}").to_ascii_lowercase())
                .collect::<Vec<_>>()
                .join(":"),
        )
    });
    normalized
}

pub fn mac_lookup_target(goal: &str) -> Option<String> {
    let mac = mac_in_goal(goal)?;
    let lower = goal.to_ascii_lowercase();
    (lower.contains("ip")
        || goal.contains("ホスト")
        || goal.contains("応答")
        || goal.contains("疎通")
        || lower.contains("ping"))
    .then_some(mac)
}

pub fn requires_ping(goal: &str) -> bool {
    goal.contains("応答") || goal.contains("疎通") || goal.to_ascii_lowercase().contains("ping")
}

pub fn local_arp_mac_target(goal: &str) -> Option<String> {
    let lower = goal.to_ascii_lowercase();
    let is_local = [
        "localhost",
        "127.0.0.1",
        "::1",
        "local",
        "ローカル",
        "自機",
        "このpc",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    if !is_local || !lower.contains("arp") {
        return None;
    }
    mac_in_goal(goal)
}

/// Returns the MAC address from an ARP-table lookup request.  Unlike
/// `local_arp_mac_target`, this also covers registered network devices (for
/// example, "NakaokuGW のARPテーブル").
pub fn arp_mac_target(goal: &str) -> Option<String> {
    let lower = goal.to_ascii_lowercase();
    if !lower.contains("arp") {
        return None;
    }
    mac_in_goal(goal)
}

/// Build the first `get_state` action for an ARP MAC lookup on a registered
/// device, and finish from that observation on the next planner turn.
pub fn plan_device_arp_mac_lookup(
    network_state: &NetworkState,
    goal: &str,
    registered_devices: &[String],
) -> Option<Decision> {
    let mac = arp_mac_target(goal)?;
    let lower_goal = goal.to_ascii_lowercase();
    let device = registered_devices
        .iter()
        .find(|candidate| {
            let name = candidate.trim();
            !name.is_empty() && lower_goal.contains(&name.to_ascii_lowercase())
        })
        .or_else(|| registered_devices.first())?;

    let mut decision = Decision {
        id: uuid::Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        action_type: crate::state::events::ActionType::Observe,
        objective: format!("{device} のARPテーブルで MAC {mac} を確認する"),
        tool: Some("get_state".to_string()),
        target: Some(device.clone()),
        parameters: serde_json::json!({"device": device, "resource": "arp", "mac": mac}),
        reason: vec!["指定機器のARPテーブルでMACアドレスを確認する".to_string()],
        expected_observation: vec![format!("{device} の最新のARPエントリ")],
        final_answer: None,
    };

    let observation = network_state
        .observed
        .observations
        .iter()
        .rev()
        .find(|observation| {
            observation.source.tool_name.as_deref() == Some("get_state")
                && observation
                    .source
                    .parameters
                    .as_ref()
                    .is_some_and(|parameters| {
                        parameters
                            .get("device")
                            .and_then(serde_json::Value::as_str)
                            .is_some_and(|observed| observed.eq_ignore_ascii_case(device))
                            && parameters
                                .get("resource")
                                .and_then(serde_json::Value::as_str)
                                == Some("arp")
                    })
        });
    if let Some(observation) = observation {
        let normalized_mac = mac.replace('-', ":").to_ascii_lowercase();
        let answer = serde_json::from_str::<serde_json::Value>(&observation.raw)
            .ok()
            .and_then(|value| value.get("arp_table").and_then(serde_json::Value::as_array).cloned())
            .map(|entries| {
                let matches: Vec<_> = entries.iter().filter(|entry| {
                    entry.get("mac_address").and_then(serde_json::Value::as_str)
                        .is_some_and(|observed| observed.eq_ignore_ascii_case(&normalized_mac))
                }).collect();
                if matches.is_empty() {
                    format!("{device} のARPテーブルに MAC {normalized_mac} は存在しません。")
                } else {
                    let ips: Vec<_> = matches.iter().filter_map(|entry| {
                        entry.get("ip_address").and_then(serde_json::Value::as_str)
                    }).collect();
                    format!("{device} のARPテーブルに MAC {normalized_mac} は存在します。対応IP: {}。", ips.join(", "))
                }
            })
            .unwrap_or_else(|| format!("{device} のARPテーブルを取得または解析できず、MAC {normalized_mac} の有無は判定できません。観測結果: {}", observation.raw));
        finish_with_evidence_answer(&mut decision, goal, answer);
    }
    Some(decision)
}

pub fn plan_local_arp_mac_lookup(network_state: &NetworkState, goal: &str) -> Option<Decision> {
    let mac = local_arp_mac_target(goal)?;
    let mut decision = Decision {
        id: uuid::Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        action_type: crate::state::events::ActionType::Observe,
        objective: format!("localhost のARPテーブルで MAC {mac} を確認する"),
        tool: Some("get_state".to_string()),
        target: Some("localhost".to_string()),
        parameters: serde_json::json!({"device":"localhost","resource":"arp","mac":mac}),
        reason: vec!["指定された対象はローカルホストのARPテーブル".to_string()],
        expected_observation: vec!["localhost の最新のARPエントリ".to_string()],
        final_answer: None,
    };
    let observation = network_state
        .observed
        .observations
        .iter()
        .rev()
        .find(|observation| {
            observation.source.tool_name.as_deref() == Some("get_state")
                && observation
                    .source
                    .parameters
                    .as_ref()
                    .is_some_and(|parameters| {
                        parameters.get("device").and_then(serde_json::Value::as_str)
                            == Some("localhost")
                            && parameters
                                .get("resource")
                                .and_then(serde_json::Value::as_str)
                                == Some("arp")
                    })
        });
    if let Some(observation) = observation {
        let answer = match serde_json::from_str::<serde_json::Value>(&observation.raw)
            .ok()
            .and_then(|value| value.get("arp_table").and_then(serde_json::Value::as_array).cloned())
        {
            Some(entries) => {
                let mac = mac.replace('-', ":").to_ascii_lowercase();
                let matches: Vec<_> = entries.iter().filter(|entry| {
                    entry.get("mac_address").and_then(serde_json::Value::as_str)
                        .is_some_and(|observed| observed.eq_ignore_ascii_case(&mac))
                }).collect();
                if matches.is_empty() {
                    format!("localhost の取得済みARPテーブルに MAC {mac} は存在しません。")
                } else {
                    let ips: Vec<_> = matches.iter().filter_map(|entry| {
                        entry.get("ip_address").and_then(serde_json::Value::as_str)
                    }).collect();
                    format!("localhost のARPテーブルに MAC {mac} は存在します。対応IP: {}。", ips.join(", "))
                }
            }
            None => format!("localhost のARPテーブルを取得または解析できず、MAC {mac} の有無は判定できません。観測結果: {}", observation.raw),
        };
        finish_with_evidence_answer(&mut decision, goal, answer);
    }
    Some(decision)
}

pub fn set_mac_graph_lookup(decision: &mut Decision, mac: &str, reason: &str) {
    decision.action_type = crate::state::events::ActionType::Observe;
    decision.objective = format!("GraphからMAC {mac} に対応するIPを検索する");
    decision.tool = Some("find_ip_by_mac".to_string());
    decision.target = None;
    decision.parameters = serde_json::json!({"mac": mac});
    decision.reason = vec![reason.to_string()];
    decision.expected_observation = vec!["IPアドレスと観測機器".to_string()];
    decision.final_answer = None;
}

/// Keep MAC-to-IP investigation moving when a graph-only query has no device
/// candidate. A MAC address does not identify an IP subnet, so registered
/// devices must be inspected before asking the user for topology details.
pub fn recover_mac_to_ip_lookup(
    decision: &mut Decision,
    network_state: &NetworkState,
    goal: &str,
    registered_devices: &[String],
) {
    let Some(mac) = mac_lookup_target(goal) else {
        return;
    };
    if crate::harness::intent::is_configuration_change_request(goal) {
        return;
    }

    let observations = &network_state.observed.observations;
    if observations.is_empty() {
        set_mac_graph_lookup(decision, &mac, "既存のGraph観測をMACで照合する");
        return;
    }
    let latest_find = observations
        .iter()
        .enumerate()
        .rev()
        .find(|(_, observation)| observation.source.tool_name.as_deref() == Some("find_ip_by_mac"));
    if let Some((_, observation)) = latest_find {
        if let Ok(result) = serde_json::from_str::<serde_json::Value>(&observation.raw) {
            if result.get("found").and_then(serde_json::Value::as_bool) == Some(true) {
                if requires_ping(goal) {
                    if let Some(ip) = result["matches"]
                        .as_array()
                        .and_then(|matches| matches.first())
                        .and_then(|entry| entry.get("ip_address"))
                        .and_then(serde_json::Value::as_str)
                    {
                        let ping = observations
                            .iter()
                            .enumerate()
                            .rev()
                            .find(|(index, entry)| {
                                *index > latest_find.expect("find observation exists").0
                                    && entry.source.tool_name.as_deref()
                                        == Some("self_network_ping")
                                    && entry
                                        .source
                                        .parameters
                                        .as_ref()
                                        .and_then(|parameters| parameters.get("host"))
                                        .and_then(serde_json::Value::as_str)
                                        == Some(ip)
                            });
                        if let Some((_, ping)) = ping {
                            finish_with_evidence_answer(
                                decision,
                                goal,
                                format!("MAC {mac} のIPは {ip}。応答確認の結果: {}", ping.raw),
                            );
                        } else {
                            decision.action_type = crate::state::events::ActionType::Observe;
                            decision.objective = format!("IP {ip} の応答を確認する");
                            decision.tool = Some("self_network_ping".to_string());
                            decision.target = None;
                            decision.parameters = serde_json::json!({"host":ip});
                            decision.reason = vec!["Graphで特定したIPにPingを送る".to_string()];
                            decision.expected_observation = vec!["応答の有無".to_string()];
                            decision.final_answer = None;
                        }
                        return;
                    }
                }
                decision.action_type = crate::state::events::ActionType::Finish;
                decision.objective = format!("MAC {mac} に対応するIPを報告する");
                decision.tool = None;
                decision.target = None;
                decision.parameters = serde_json::Value::Null;
                decision.reason.clear();
                decision.expected_observation.clear();
                decision.final_answer = Some(result["matches"].to_string());
                return;
            }
        }
    }

    let latest_arp = observations
        .iter()
        .enumerate()
        .rev()
        .find(|(_, observation)| {
            observation.source.tool_name.as_deref() == Some("get_state")
                && observation
                    .source
                    .parameters
                    .as_ref()
                    .and_then(|parameters| parameters.get("resource"))
                    .and_then(serde_json::Value::as_str)
                    == Some("arp")
        });
    let needs_find = latest_arp
        .is_some_and(|(index, _)| latest_find.is_none_or(|(find_index, _)| index > find_index));
    if needs_find {
        set_mac_graph_lookup(decision, &mac, "最新のARP観測をGraphで照合する");
        return;
    }

    let observed_devices: Vec<&str> = observations
        .iter()
        .filter(|observation| observation.source.tool_name.as_deref() == Some("get_state"))
        .filter_map(|observation| {
            let parameters = observation.source.parameters.as_ref()?;
            (parameters.get("resource")?.as_str()? == "arp")
                .then(|| parameters.get("device").and_then(serde_json::Value::as_str))
                .flatten()
        })
        .collect();
    if let Some(device) = registered_devices.iter().find(|device| {
        !observed_devices
            .iter()
            .any(|observed| observed.eq_ignore_ascii_case(device))
    }) {
        decision.action_type = crate::state::events::ActionType::Observe;
        decision.objective = format!("{device} のARPを取得してMAC {mac} を調べる");
        decision.tool = Some("get_state".to_string());
        decision.target = Some(device.clone());
        decision.parameters = serde_json::json!({"device": device, "resource": "arp"});
        decision.reason = vec!["Graphに対応するIPがないため登録機器のARPを確認する".to_string()];
        decision.expected_observation = vec!["更新されたARP観測".to_string()];
        decision.final_answer = None;
    } else if !observed_devices.is_empty() {
        let has_arp_data = observations.iter().any(|observation| {
            observation.source.tool_name.as_deref() == Some("get_state")
                && serde_json::from_str::<serde_json::Value>(&observation.raw)
                    .ok()
                    .and_then(|value| value.get("arp_table").cloned())
                    .and_then(|value| value.as_array().cloned())
                    .is_some()
        });
        if has_arp_data {
            finish_with_evidence_answer(
                decision,
                goal,
                format!(
                    "MAC {mac} に対応するIPは、確認した登録機器のARP観測には見つからなかった。"
                ),
            );
        } else {
            decision.action_type = crate::state::events::ActionType::AskHuman;
            decision.objective =
                "登録機器からARPを取得できませんでした。機器の接続状態を確認してください。"
                    .to_string();
            decision.tool = None;
            decision.target = None;
            decision.parameters = serde_json::Value::Null;
            decision.reason = vec!["登録機器のARP取得が失敗した".to_string()];
            decision.expected_observation.clear();
            decision.final_answer = None;
        }
    } else if latest_find.is_none() {
        set_mac_graph_lookup(
            decision,
            &mac,
            "登録機器がないため既存のGraph観測を確認する",
        );
    } else {
        decision.action_type = crate::state::events::ActionType::AskHuman;
        decision.objective = format!(
            "MAC {mac} に対応するIPを確認できませんでした。ARPを取得できる機器を登録してください。"
        );
        decision.tool = None;
        decision.target = None;
        decision.parameters = serde_json::Value::Null;
        decision.reason = vec!["Graphに一致がなく、登録機器もない".to_string()];
        decision.expected_observation.clear();
        decision.final_answer = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::decision::parse_decision_from_json;

    #[test]
    fn localhost_arp_mac_lookup_uses_local_state_and_finishes_from_its_entries() {
        let goal = "localhost のARPテーブルにea:f1:92:50:7b:c3は存在する？";
        assert!(
            local_arp_mac_target("NakaokuGW のARPテーブルにea:f1:92:50:7b:c3は存在する？")
                .is_none()
        );
        let mut state = NetworkState::with_goal(goal.to_string());
        let first = plan_local_arp_mac_lookup(&state, goal).unwrap();
        assert_eq!(first.tool.as_deref(), Some("get_state"));
        assert_eq!(
            first.parameters,
            serde_json::json!({"device":"localhost","resource":"arp","mac":"ea:f1:92:50:7b:c3"})
        );

        add_tool_result(
            &mut state,
            "get_state",
            Some("localhost"),
            r#"{"arp_table":[{"ip_address":"192.0.2.5","mac_address":"EA:F1:92:50:7B:C3"}]}"#,
        );
        let found = plan_local_arp_mac_lookup(&state, goal).unwrap();
        assert_eq!(found.action_type, crate::state::events::ActionType::Finish);
        assert!(found.final_answer.unwrap().contains("192.0.2.5"));

        state.observed.observations.last_mut().unwrap().raw = r#"{"arp_table":[]}"#.to_string();
        let absent = plan_local_arp_mac_lookup(&state, goal).unwrap();
        assert!(absent.final_answer.unwrap().contains("存在しません"));
    }

    #[test]
    fn registered_device_arp_mac_lookup_always_supplies_get_state_arguments() {
        let goal = "NakaokuGW のARPテーブルにea:f1:92:50:7b:c3は存在する？";
        let state = NetworkState::with_goal(goal.to_string());
        let decision = plan_device_arp_mac_lookup(
            &state,
            goal,
            &["NakaokuGW".to_string(), "F220".to_string()],
        )
        .unwrap();
        assert_eq!(decision.tool.as_deref(), Some("get_state"));
        assert_eq!(decision.target.as_deref(), Some("NakaokuGW"));
        assert_eq!(
            decision.parameters,
            serde_json::json!({
                "device": "NakaokuGW",
                "resource": "arp",
                "mac": "ea:f1:92:50:7b:c3"
            })
        );
    }

    #[test]
    fn failed_local_arp_observation_is_not_reported_as_absent() {
        let goal = "localhost のARPテーブルにea:f1:92:50:7b:c3は存在する？";
        let mut state = NetworkState::with_goal(goal.to_string());
        add_tool_result(
            &mut state,
            "get_state",
            Some("localhost"),
            "Execution error: arp failed",
        );
        let decision = plan_local_arp_mac_lookup(&state, goal).unwrap();
        assert!(decision.final_answer.unwrap().contains("判定できません"));
    }

    fn add_tool_result(state: &mut NetworkState, tool: &str, device: Option<&str>, raw: &str) {
        state.apply_observation(crate::state::events::Observation {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            raw: raw.to_string(),
            parsed: None,
            source: crate::state::events::ObservationSource {
                device: device.map(str::to_string),
                command: None,
                tool_name: Some(tool.to_string()),
                tool_kind: None,
                parameters: device.map(|name| serde_json::json!({"device":name,"resource":"arp"})),
            },
            provenance: crate::state::events::Provenance {
                origin: crate::state::events::ProvenanceOrigin::Tool,
                confidence: Some(1.0),
            },
        });
    }

    #[test]
    fn host_reachability_goal_constrains_query_and_pings_resolved_ip() {
        let goal = "ea:f1:92:50:7b:c3のホストを調べて応答があるか確認してください";
        let mut state = NetworkState::with_goal(goal.to_string());
        let mut decision = parse_decision_from_json(
            r#"{"action_type":"OBSERVE","objective":"MACのホストを検索","tool":"query_network_graph","parameters":{"query":"ea:f1:92:50:7b:c3 のホスト情報とIPアドレス","mac":"ea:f1:92:50:7b:c3"}}"#,
        ).unwrap();
        recover_mac_to_ip_lookup(&mut decision, &state, goal, &["NakaokuGW".to_string()]);
        assert_eq!(decision.tool.as_deref(), Some("find_ip_by_mac"));
        add_tool_result(
            &mut state,
            "find_ip_by_mac",
            None,
            r#"{"found":true,"matches":[{"device_name":"NakaokuGW","ip_address":"10.0.0.10","mac_address":"ea:f1:92:50:7b:c3"}]}"#,
        );
        recover_mac_to_ip_lookup(&mut decision, &state, goal, &["NakaokuGW".to_string()]);
        assert_eq!(decision.tool.as_deref(), Some("self_network_ping"));
        assert_eq!(decision.parameters["host"], "10.0.0.10");
        add_tool_result(
            &mut state,
            "self_network_ping",
            None,
            "10.0.0.10から応答あり",
        );
        state
            .observed
            .observations
            .last_mut()
            .unwrap()
            .source
            .parameters = Some(serde_json::json!({"host":"10.0.0.10"}));
        recover_mac_to_ip_lookup(&mut decision, &state, goal, &["NakaokuGW".to_string()]);
        assert_eq!(
            decision.action_type,
            crate::state::events::ActionType::Finish
        );
        assert!(decision
            .final_answer
            .as_deref()
            .unwrap()
            .contains("応答あり"));
    }

    #[test]
    fn short_mac_octet_uses_endpoint_lookup_and_then_ping() {
        let goal = "0:2b:f5:3c:cc:7cから応答があるかチェック";
        let mac = "00:2b:f5:3c:cc:7c";
        assert_eq!(mac_lookup_target(goal).as_deref(), Some(mac));
        let mut state = NetworkState::with_goal(goal.to_string());
        let mut decision = parse_decision_from_json(
            r#"{"action_type":"OBSERVE","objective":"Graphを検索","tool":"query_network_graph","parameters":{"query":"0:2b:f5:3c:cc:7c"}}"#,
        )
        .unwrap();
        recover_mac_to_ip_lookup(&mut decision, &state, goal, &[]);
        assert_eq!(decision.tool.as_deref(), Some("find_ip_by_mac"));
        assert_eq!(decision.parameters["mac"], mac);

        add_tool_result(
            &mut state,
            "find_ip_by_mac",
            None,
            r#"{"found":true,"matches":[{"ip_address":"192.0.2.7"}]}"#,
        );
        recover_mac_to_ip_lookup(&mut decision, &state, goal, &[]);
        assert_eq!(decision.tool.as_deref(), Some("self_network_ping"));
        assert_eq!(decision.parameters["host"], "192.0.2.7");
    }

    #[test]
    fn short_mac_octet_does_not_match_part_of_a_longer_address() {
        assert!(mac_in_goal("000:2b:f5:3c:cc:7cから応答があるか").is_none());
        assert!(mac_in_goal("0:2b:f5:3c:cc:7c:eeから応答があるか").is_none());
    }

    #[test]
    fn cisco_mac_format_uses_the_same_endpoint_lookup() {
        let goal = "aaaa.cccc.ddddから応答があるかチェック";
        let normalized = "aa:aa:cc:cc:dd:dd";
        assert_eq!(mac_lookup_target(goal).as_deref(), Some(normalized));

        let state = NetworkState::with_goal(goal.to_string());
        let mut decision = parse_decision_from_json(
            r#"{"action_type":"OBSERVE","objective":"Graphを検索","tool":"query_network_graph","parameters":{"query":"aaaa.cccc.dddd"}}"#,
        )
        .unwrap();
        recover_mac_to_ip_lookup(&mut decision, &state, goal, &[]);
        assert_eq!(decision.tool.as_deref(), Some("find_ip_by_mac"));
        assert_eq!(decision.parameters["mac"], normalized);
    }

    #[test]
    fn mac_goal_uses_structured_lookup_instead_of_text_query() {
        let goal = "ea:f1:92:50:7b:c3のIPアドレスは？";
        let state = NetworkState::with_goal(goal.to_string());
        let mut decision = parse_decision_from_json(
            r#"{"action_type":"OBSERVE","objective":"MACを探す","tool":"query_network_graph","parameters":{"mac":"ea:f1:92:50:7b:c3","query":"ea:f1:92:50:7b:c3のIPアドレス"}}"#,
        ).unwrap();
        recover_mac_to_ip_lookup(&mut decision, &state, goal, &["NakaokuGW".to_string()]);
        assert_eq!(decision.tool.as_deref(), Some("find_ip_by_mac"));
        assert_eq!(
            decision.parameters,
            serde_json::json!({"mac":"ea:f1:92:50:7b:c3"})
        );
    }

    #[test]
    fn empty_mac_graph_result_fetches_registered_arp_before_asking_human() {
        let goal = "ea:f1:92:50:7b:c3 のIPアドレスは？";
        let mut state = NetworkState::with_goal(goal.to_string());
        add_tool_result(
            &mut state,
            "query_network_graph",
            None,
            r#"{"candidate_devices":[]}"#,
        );
        let mut decision = parse_decision_from_json(
            r#"{"action_type":"ASK_HUMAN","objective":"対象機器を教えてください"}"#,
        )
        .unwrap();
        let devices = vec!["NakaokuGW".to_string(), "F220".to_string()];

        recover_mac_to_ip_lookup(&mut decision, &state, goal, &devices);
        assert_eq!(decision.tool.as_deref(), Some("get_state"));
        assert_eq!(decision.parameters["device"], "NakaokuGW");

        add_tool_result(
            &mut state,
            "get_state",
            Some("NakaokuGW"),
            "ARP observation",
        );
        recover_mac_to_ip_lookup(&mut decision, &state, goal, &devices);
        assert_eq!(decision.tool.as_deref(), Some("find_ip_by_mac"));
        add_tool_result(
            &mut state,
            "find_ip_by_mac",
            None,
            r#"{"found":false,"matches":[]}"#,
        );
        recover_mac_to_ip_lookup(&mut decision, &state, goal, &devices);
        assert_eq!(decision.tool.as_deref(), Some("get_state"));
        assert_eq!(decision.parameters["device"], "F220");
        add_tool_result(&mut state, "get_state", Some("F220"), "ARP observation");
        recover_mac_to_ip_lookup(&mut decision, &state, goal, &devices);
        assert_eq!(decision.tool.as_deref(), Some("find_ip_by_mac"));
        add_tool_result(
            &mut state,
            "find_ip_by_mac",
            None,
            r#"{"found":true,"matches":[{"device_name":"NakaokuGW","ip_address":"10.0.0.10","mac_address":"ea:f1:92:50:7b:c3"}]}"#,
        );
        recover_mac_to_ip_lookup(&mut decision, &state, goal, &devices);
        assert_eq!(
            decision.action_type,
            crate::state::events::ActionType::Finish
        );
        assert!(decision
            .final_answer
            .as_deref()
            .unwrap()
            .contains("10.0.0.10"));
    }
}
