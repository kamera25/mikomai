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

fn extract_json(raw: &str) -> Option<&str> {
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
                "roots":{"type":"array","items":{"type":"string","minLength":1},"minItems":1,"maxItems":32},
                "depth":{"type":"integer","minimum":0,"maximum":8},
                "relations":{"type":"array","items":{"type":"string","enum":["interface","bgp","vrf","route"]},"minItems":1},
                "query":{"type":"string"},"ip":{"type":"string"},"mac":{"type":"string"},
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
    use super::*;

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
