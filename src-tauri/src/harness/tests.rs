#[cfg(test)]
mod tests {
    use crate::state::*;
    use crate::validator::*;
    use crate::planner::decision::*;

    #[test]
    fn test_network_state_goal_and_observation() {
        let mut state = NetworkState::with_goal("Verify connectivity to 10.0.20.0/24".to_string());
        assert_eq!(state.desired.as_ref().unwrap().raw_goal, "Verify connectivity to 10.0.20.0/24");
        assert_eq!(state.event_log.len(), 1);

        let obs = Observation {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            raw: "% Network not in table".to_string(),
            parsed: None,
            source: ObservationSource {
                device: Some("R1".to_string()),
                command: Some("show ip route 10.0.20.0".to_string()),
                tool_name: Some("network_show".to_string()),
                tool_kind: None,
                parameters: Some(serde_json::json!({"command": "show ip route 10.0.20.0"})),
            },

            provenance: Provenance {
                origin: ProvenanceOrigin::Tool,
                confidence: Some(1.0),
            },
        };

        state.apply_observation(obs);
        assert_eq!(state.event_log.len(), 2);
        assert!(state.observed.devices.contains_key("R1"));
        let r1_fact = state.observed.devices.get("R1").unwrap();
        assert_eq!(r1_fact.raw_snapshots.get("show ip route 10.0.20.0").unwrap(), "% Network not in table");

        // Test state rebuild from event log
        let rebuilt = NetworkState::rebuild_from_log(&state.event_log);
        assert_eq!(rebuilt.desired.as_ref().unwrap().raw_goal, "Verify connectivity to 10.0.20.0/24");
        assert!(rebuilt.observed.devices.contains_key("R1"));
    }

    #[test]
    fn test_decision_parsing_and_validation() {
        let raw_json = r#"{
            "action_type": "OBSERVE",
            "objective": "Check OSPF database on R1",
            "tool": "network_show",
            "target": "R1",
            "parameters": {
                "command": "show ip ospf database"
            },
            "reason": [
                "Route is absent from R1",
                "OSPF advertisement failure is plausible"
            ],
            "expected_observation": [
                "LSA for 10.0.20.0/24"
            ]
        }"#;

        let decision = parse_decision_from_json(raw_json).expect("Failed to parse valid decision");
        assert_eq!(decision.action_type, ActionType::Observe);
        assert_eq!(decision.target.as_deref(), Some("R1"));
        assert_eq!(decision.reason.len(), 2);

        let action = SchemaValidator::validate_decision(&decision).expect("Validation should pass");
        assert_eq!(action.action_type, ActionType::Observe);
        assert_eq!(action.target.as_deref(), Some("R1"));

        assert!(PolicyValidator::validate_action(&action).is_ok());
    }

    #[test]
    fn test_policy_validator_blocked_commands() {
        let decision = Decision {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            action_type: ActionType::Configure,
            objective: "Format flash memory".to_string(),
            tool: Some("network_config".to_string()),
            target: Some("R1".to_string()),
            parameters: serde_json::json!({
                "commands": ["format flash:"]
            }),
            reason: vec![],
            expected_observation: vec![],
            final_answer: None,
        };

        let action = SchemaValidator::validate_decision(&decision).unwrap();
        let policy_res = PolicyValidator::validate_action(&action);
        assert!(policy_res.is_err());
    }

    #[test]
    fn test_ping_observation_retained_in_context() {
        let mut state = NetworkState::with_goal("Ping 192.168.1.1".to_string());

        let ping_obs = Observation {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            raw: "64 bytes from 192.168.1.1: icmp_seq=0 ttl=64 time=1.23 ms".to_string(),
            parsed: None,
            source: ObservationSource {
                device: None,
                command: None,
                tool_name: Some("self_network_ping".to_string()),
                tool_kind: None,
                parameters: Some(serde_json::json!({"host": "192.168.1.1"})),
            },
            provenance: Provenance {
                origin: ProvenanceOrigin::Tool,
                confidence: Some(1.0),
            },
        };

        state.apply_observation(ping_obs);
        assert_eq!(state.observed.observations.len(), 1);

        let context_str = state.to_prompt_context();
        assert!(context_str.contains("self_network_ping"));
        assert!(context_str.contains("64 bytes from 192.168.1.1"));
    }

    #[test]
    fn test_network_show_decision_resolution() {
        let raw_json = r#"{
            "action_type": "OBSERVE",
            "objective": "Show ip route on R1",
            "tool": "network_show",
            "target": "R1",
            "parameters": {
                "command": "show ip route"
            },
            "reason": ["Check routing table"],
            "expected_observation": ["Routing table entries"]
        }"#;

        let decision = parse_decision_from_json(raw_json).expect("Parse failed");
        let action = SchemaValidator::validate_decision(&decision).expect("Validation failed");
        assert_eq!(action.tool.as_deref(), Some("network_show"));
        assert_eq!(action.target.as_deref(), Some("R1"));
        assert_eq!(action.parameters.get("command").and_then(|v| v.as_str()), Some("show ip route"));
    }
}


