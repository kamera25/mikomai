//! Portable structured planner decisions used by native and web runtimes.
use crate::domain::ActionType;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlannerDecision {
    #[serde(rename = "action_type")]
    pub action: ActionType,
    pub objective: String,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub parameters: serde_json::Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reason: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_observation: Vec<String>,
    #[serde(default)]
    pub final_answer: Option<String>,
}

impl PlannerDecision {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let body = extract_json(raw)
            .ok_or_else(|| "planner did not return a JSON decision".to_string())?;
        let mut value: serde_json::Value =
            serde_json::from_str(body).map_err(|error| format!("invalid planner JSON: {error}"))?;
        if let Some(inner) = value.get("decision").or_else(|| value.get("Decision")) {
            if inner.is_object() {
                value = inner.clone();
            }
        }
        let mut decision: Self = serde_json::from_value(value)
            .map_err(|error| format!("invalid planner decision: {error}"))?;
        if decision.objective.trim().is_empty() {
            decision.objective = "処理完了/状態確認".to_string();
        }
        if decision.action == ActionType::Finish {
            decision.reason.clear();
        }
        if !decision.parameters.is_null() && !decision.parameters.is_object() {
            return Err("planner decision parameters must be an object or null".into());
        }
        // Some local models nest read-only probe tools alongside their arguments.
        // Recover these known tools; the normal allow-list still validates them.
        if matches!(decision.action, ActionType::Observe | ActionType::Verify) {
            let nested = decision.parameters.get("tool").and_then(serde_json::Value::as_str);
            if decision.tool.is_none() && matches!(nested, Some("self_network_test_connection" | "self_network_test_net_connection" | "self_network_ping" | "self_network_traceroute")) {
                decision.tool = nested.map(str::to_owned);
                decision.parameters.as_object_mut().unwrap().remove("tool");
            }
            if matches!(decision.tool.as_deref(), Some("self_network_ping" | "self_network_traceroute")) && decision.parameters.get("host").is_none() {
                let host = decision.parameters.get("device").and_then(serde_json::Value::as_str)
                    .or(decision.target.as_deref()).filter(|host| !host.trim().is_empty() && *host != "localhost").map(str::to_owned);
                if let Some(host) = host { decision.parameters["host"] = host.into(); }
            }
            if matches!(decision.tool.as_deref(), Some("self_network_test_connection" | "self_network_test_net_connection")) {
                if decision.parameters.is_null() { decision.parameters = serde_json::json!({}); }
                let host = decision.parameters.get("host").and_then(serde_json::Value::as_str)
                    .or_else(|| decision.parameters.get("ip").and_then(serde_json::Value::as_str))
                    .or_else(|| decision.parameters.get("device").and_then(serde_json::Value::as_str))
                    .or(decision.target.as_deref()).map(str::to_owned);
                if let Some(host) = host { decision.parameters["host"] = host.into(); }
                crate::service_ports::normalize_parameters(&mut decision.parameters)?;
            }
        }
        Ok(decision)
    }

    pub fn validate(&self, allowed_tools: &[String]) -> Result<(), String> {
        if self.objective.trim().is_empty() {
            return Err("decision objective cannot be empty".into());
        }
        if let Some(tool) = self.tool.as_deref() {
            if !allowed_tools.iter().any(|allowed| allowed == tool) {
                return Err(format!("unknown tool: {tool}"));
            }
        }
        if matches!(self.tool.as_deref(), Some("self_network_test_connection" | "self_network_test_net_connection")) {
            if !self.parameters["port"].as_u64().is_some_and(|p| (1..=65535).contains(&p)) {
                return Err("TCP port check requires port between 1 and 65535".into());
            }
            if self.parameters.get("protocol").is_some_and(|p| p.as_str().is_none_or(|p| !p.eq_ignore_ascii_case("tcp"))) {
                return Err("port check supports TCP only; UDP cannot be checked with a TCP connection".into());
            }
            if self.parameters["host"].as_str().is_none_or(|host| host.trim().is_empty()) {
                return Err("TCP port check requires a host".into());
            }
        }
        if self.tool.as_deref() == Some("get_state") && self.parameters["resource"] == "interfaces"
            && self.target.as_deref().or_else(|| self.parameters["device"].as_str()).is_none_or(|target| target.trim().is_empty()) {
            return Err("interface state requires a target device".into());
        }
        match self.action {
            ActionType::Observe | ActionType::Verify
                if self.tool.is_none() && self.target.is_none() =>
            {
                Err(format!(
                    "action {:?} requires a tool or target",
                    self.action
                ))
            }
            ActionType::Configure | ActionType::Rollback if self.target.is_none() => {
                Err(format!("action {:?} requires a target device", self.action))
            }
            _ => Ok(()),
        }
    }

    pub fn to_core_decision(&self) -> crate::domain::Decision {
        crate::domain::Decision {
            id: Uuid::new_v4(),
            action: self.action,
            objective: self.objective.clone(),
            tool: self.tool.clone(),
            target: self.target.clone(),
            arguments: self.parameters.clone(),
            final_answer: self.final_answer.clone(),
        }
    }
}

pub(crate) fn extract_json(raw: &str) -> Option<&str> {
    if let Some(start) = raw.find('{') {
        let bytes = raw.as_bytes();
        let mut depth = 0i32;
        let mut quoted = false;
        let mut escaped = false;
        for index in start..bytes.len() {
            let byte = bytes[index];
            if quoted {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    quoted = false;
                }
                continue;
            }
            match byte {
                b'"' => quoted = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return std::str::from_utf8(&bytes[start..=index]).ok();
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// Builds the model's structured decision output schema from runtime-owned
/// device and tool names. Device secrets must never be passed here.
pub fn build_decision_schema(devices: &[String], tools: &[String]) -> String {
    let tool_enum: Vec<serde_json::Value> = tools
        .iter()
        .cloned()
        .map(serde_json::Value::String)
        .chain([serde_json::Value::Null])
        .collect();
    let target_schema = if devices.is_empty() {
        serde_json::json!({"type":["string","null"]})
    } else {
        serde_json::json!({"anyOf":[{"type":"string","enum":devices},{"type":"null"}]})
    };
    serde_json::json!({
        "type":"object",
        "properties":{
            "action_type":{"type":"string","enum":["OBSERVE","VERIFY","CONFIGURE","ROLLBACK","ASK_HUMAN","FINISH"]},
            "objective":{"type":"string"},
            "tool":{"enum":tool_enum},
            "target":target_schema,
            "parameters":{"type":"object","properties":{
                "device":target_schema.clone(),
                "resource":{"type":"string","enum":["arp","routes","interfaces","lldp","mac_table","bgp","ospf"]},
                "refresh":{"type":"boolean"},
                "roots":{"type":"array","items":{"type":"string","minLength":1},"minItems":1,"maxItems":32},
                "depth":{"type":"integer","minimum":0,"maximum":8},
                "relations":{"type":"array","items":{"type":"string","enum":["interface","bgp","vrf","route"]},"minItems":1},
                "service":{"type":"string"},"query":{"type":"string"},"ip":{"type":"string"},"mac":{"type":"string"},
                "port":{"type":["integer","string"],"minimum":1,"maximum":65535,"description":"Port number or service name; e.g. ssh, dns, tcp/22"},
                "protocol":{"type":"string","enum":["tcp"]},
                "command":{"type":"string"},"host":{"type":"string"},"id":{"type":"string"},
                "intent":{"type":"string","enum":["analyze_broadcast","analyze_dhcp_response","prepare_dhcp_request","dhcp_request_probe"]},
                "frame_hex":{"type":"string"},"client_mac":{"type":"string"},"transaction_id":{"type":"string"},
                "requested_ip":{"type":"string"},"server_identifier":{"type":"string"},"interface":{"type":"string"},
                "vlan":{"type":"integer","minimum":1,"maximum":4094}
            }},
            "reason":{"type":"array","items":{"type":"string"}},
            "expected_observation":{"type":"array","items":{"type":"string"}},
            "final_answer":{"type":["string","null"]}
        },
        "required":["action_type","objective"]
    }).to_string()
}

/// Builds the runtime schema and narrows MAC address queries to the legacy
/// deterministic ARP lookup tools.
pub fn build_goal_decision_schema(devices: &[String], tools: &[String], goal: &str) -> String {
    let mut schema: serde_json::Value =
        serde_json::from_str(&build_decision_schema(devices, tools))
            .expect("generated decision schema is valid JSON");
    if let Some(host) = crate::agent::ping_statistics_target(goal) {
        schema["properties"]["tool"] = serde_json::json!({"enum":["self_network_ping", null]});
        schema["properties"]["parameters"] = serde_json::json!({
            "type":"object", "properties":{
                "host":{"type":"string","enum":[host]},
                "count":{"type":"integer","minimum":1,"maximum":100},
                "size":{"type":"integer","minimum":1,"maximum":65507},
                "dont_fragment":{"type":"boolean"}
            }
        });
    }
    if let Some(mac) = crate::dispatch::arp_mac_target(goal)
        .or_else(|| {
            crate::dispatch::mac_address_in_goal(goal)
                .filter(|_| crate::dispatch::mac_lookup_requires_ping(goal))
        })
        .filter(|_| !crate::intent::is_configuration_change_request(goal))
    {
        let allowed: Vec<&str> = if crate::dispatch::mac_lookup_requires_ping(goal) {
            vec!["find_ip_by_mac", "get_state", "self_network_ping"]
        } else {
            vec!["find_ip_by_mac", "get_state"]
        };
        schema["properties"]["tool"] =
            serde_json::json!({"anyOf":[{"type":"string","enum":allowed},{"type":"null"}]});
        let parameters = &mut schema["properties"]["parameters"];
        if let Some(properties) = parameters["properties"].as_object_mut() {
            properties
                .retain(|key, _| ["device", "resource", "mac", "host"].contains(&key.as_str()));
            properties.insert(
                "mac".into(),
                serde_json::json!({"type":"string","enum":[mac]}),
            );
            properties.insert(
                "resource".into(),
                serde_json::json!({"type":"string","enum":["arp"]}),
            );
        }
        parameters["additionalProperties"] = serde_json::Value::Bool(false);
    }
    schema.to_string()
}

#[cfg(test)]
mod tests {

    #[test]
    fn normalizes_service_names_in_planner_arguments() {
        let allowed = vec!["self_network_test_connection".into()];
        for parameters in [serde_json::json!({"host":"router","port":"ssh"}), serde_json::json!({"host":"router","query":"tcp/22"}), serde_json::json!({"host":"router","service":"SSH"})] {
            let raw = serde_json::json!({"action_type":"VERIFY","objective":"check","tool":allowed[0],"parameters":parameters}).to_string();
            let decision = super::PlannerDecision::parse(&raw).unwrap();
            decision.validate(&allowed).unwrap();
            assert_eq!(decision.parameters["port"], 22);
            assert_eq!(decision.parameters["protocol"], "tcp");
        }
        let raw = serde_json::json!({"action_type":"VERIFY","objective":"check","tool":allowed[0],"parameters":{"host":"router","query":"udp/22"}}).to_string();
        let decision = super::PlannerDecision::parse(&raw).unwrap();
        assert_eq!(decision.parameters["protocol"], "udp");
        assert!(decision.validate(&allowed).is_err());
    }

    #[test]
    fn recovers_logged_tcp_decision_and_rejects_udp_and_bad_ports() {
        let allowed = vec!["self_network_test_connection".into()];
        let decision = super::PlannerDecision::parse(r#"{"action_type":"VERIFY","objective":"check","parameters":{"device":"NakaokuGW","tool":"self_network_test_connection","ip":null,"query":"22/tcp"}}"#).unwrap();
        decision.validate(&allowed).unwrap();
        assert_eq!(decision.tool.as_deref(), Some("self_network_test_connection"));
        assert_eq!(decision.parameters["host"], "NakaokuGW");
        assert_eq!(decision.parameters["port"], 22);
        for (port, protocol) in [(0, "tcp"), (65536, "tcp"), (53, "udp")] {
            let raw = serde_json::json!({"action_type":"VERIFY","objective":"check","tool":"self_network_test_connection","parameters":{"host":"router","port":port,"protocol":protocol}}).to_string();
            assert!(super::PlannerDecision::parse(&raw).unwrap().validate(&allowed).is_err());
        }
        let schema: serde_json::Value = serde_json::from_str(&super::build_decision_schema(&[], &allowed)).unwrap();
        assert_eq!(schema["properties"]["parameters"]["properties"]["port"]["maximum"], 65535);
    }

    use super::*;

    #[test]
    fn nested_ping_and_natural_statistics_schema_remain_read_only() {
        let decision = PlannerDecision::parse(r#"{"action_type":"OBSERVE","objective":"packet statistics","parameters":{"tool":"self_network_ping","device":"127.0.0.1"}}"#).unwrap();
        decision.validate(&["self_network_ping".into()]).unwrap();
        assert_eq!(decision.tool.as_deref(), Some("self_network_ping"));
        assert_eq!(decision.parameters["host"], "127.0.0.1");
        let schema: serde_json::Value = serde_json::from_str(&build_goal_decision_schema(&[], &["self_network_ping".into(), "self_network_test_connection".into()], "127.0.0.1への疎通を調べて、損失率を教えてください")).unwrap();
        assert_eq!(schema["properties"]["tool"]["enum"], serde_json::json!(["self_network_ping", null]));
        assert_eq!(schema["properties"]["parameters"]["properties"]["host"]["enum"], serde_json::json!(["127.0.0.1"]));
        assert!(schema["properties"]["parameters"]["properties"].get("port").is_none());
    }

    #[test]
    fn parses_wrapped_finish_and_hides_planner_reason() {
        let decision = PlannerDecision::parse(r#"```json
{"decision":{"action_type":"FINISH","objective":"done","reason":["private"],"final_answer":"answer"}}
```"#).unwrap();
        assert_eq!(decision.action, ActionType::Finish);
        assert!(decision.reason.is_empty());
        assert_eq!(decision.final_answer.as_deref(), Some("answer"));
    }

    #[test]
    fn rejects_unknown_tools_and_malformed_action_requirements() {
        let decision = PlannerDecision::parse(
            r#"{"action_type":"OBSERVE","objective":"check","tool":"fake"}"#,
        )
        .unwrap();
        assert!(decision
            .validate(&["get_state".into()])
            .unwrap_err()
            .contains("unknown tool"));
    }

    #[test]
    fn creates_runtime_schema_without_secret_values() {
        let schema = build_decision_schema(&["router-1".into()], &["get_state".into()]);
        let value: serde_json::Value = serde_json::from_str(&schema).unwrap();
        assert!(value["properties"]["tool"]["enum"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("get_state")));
        assert!(value["properties"]["target"]["anyOf"][0]["enum"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("router-1")));
    }

    #[test]
    fn narrows_mac_lookup_schema_to_arp_and_optional_reachability_tools() {
        let schema: serde_json::Value = serde_json::from_str(&build_goal_decision_schema(
            &["router-1".into()],
            &[
                "get_state".into(),
                "find_ip_by_mac".into(),
                "query_nw_db".into(),
            ],
            "router-1のARPテーブルにea-f1-92-50-7b-c3は存在する？",
        ))
        .unwrap();
        let tools = &schema["properties"]["tool"]["anyOf"][0]["enum"];
        assert!(tools
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("find_ip_by_mac")));
        assert!(!tools
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("query_nw_db")));
        assert_eq!(
            schema["properties"]["parameters"]["properties"]["mac"]["enum"][0],
            "ea:f1:92:50:7b:c3"
        );
    }
}
