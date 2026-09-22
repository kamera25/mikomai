use crate::planner::mac_lookup::{mac_lookup_target, requires_ping};

pub fn build_planner_schema(registered_devices: &[String]) -> String {
    let device_schema = if !registered_devices.is_empty() {
        let enum_json =
            serde_json::to_string(registered_devices).unwrap_or_else(|_| "[]".to_string());
        format!(r#"{{ "type": "string", "enum": {} }}"#, enum_json)
    } else {
        r#"{ "type": "string" }"#.to_string()
    };

    let target_schema = if !registered_devices.is_empty() {
        let enum_json =
            serde_json::to_string(registered_devices).unwrap_or_else(|_| "[]".to_string());
        format!(
            r#"{{ "anyOf": [ {{ "type": "string", "enum": {} }}, {{ "type": "null" }} ] }}"#,
            enum_json
        )
    } else {
        r#"{ "type": ["string", "null"] }"#.to_string()
    };

    format!(
        r#"{{
  "type": "object",
  "properties": {{
    "action_type": {{
      "type": "string",
      "enum": ["OBSERVE", "VERIFY", "CONFIGURE", "ROLLBACK", "ASK_HUMAN", "FINISH"]
    }},
    "objective": {{ "type": "string" }},
    "tool": {{ "type": ["string", "null"] }},
    "target": {target_schema},
    "parameters": {{
      "type": "object",
      "properties": {{
        "device": {device_schema},
        "resource": {{
          "type": "string",
          "enum": ["arp", "routes", "interfaces", "lldp", "mac_table", "bgp", "ospf"]
        }},
        "query": {{ "type": "string" }},
        "ip": {{ "type": "string" }},
        "mac": {{ "type": "string" }},
        "command": {{ "type": "string" }},
        "host": {{ "type": "string" }},
        "id": {{ "type": "string" }},
        "intent": {{ "type": "string", "enum": ["analyze_broadcast", "analyze_dhcp_response", "prepare_dhcp_request", "dhcp_request_probe"] }},
        "frame_hex": {{ "type": "string" }},
        "client_mac": {{ "type": "string" }},
        "transaction_id": {{ "type": "string" }},
        "requested_ip": {{ "type": "string" }},
        "server_identifier": {{ "type": "string" }},
        "interface": {{ "type": "string" }},
        "vlan": {{ "type": "integer", "minimum": 1, "maximum": 4094 }}
      }}
    }},
    "reason": {{
      "type": "array",
      "items": {{ "type": "string" }}
    }},
    "expected_observation": {{
      "type": "array",
      "items": {{ "type": "string" }}
    }},
    "final_answer": {{ "type": ["string", "null"] }}
  }},
  "required": ["action_type", "objective"]
}}"#
    )
}

pub const DECISION_JSON_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "action_type": {
      "type": "string",
      "enum": ["OBSERVE", "VERIFY", "CONFIGURE", "ROLLBACK", "ASK_HUMAN", "FINISH"]
    },
    "objective": { "type": "string" },
    "tool": { "type": ["string", "null"] },
    "target": { "type": ["string", "null"] },
    "parameters": {
      "type": "object",
      "properties": {
        "device": { "type": "string" },
        "resource": {
          "type": "string",
          "enum": ["arp", "routes", "interfaces", "lldp", "mac_table", "bgp", "ospf"]
        },
        "query": { "type": "string" },
        "ip": { "type": "string" },
        "mac": { "type": "string" },
        "command": { "type": "string" },
        "host": { "type": "string" },
        "id": { "type": "string" },
        "intent": { "type": "string", "enum": ["analyze_broadcast", "analyze_dhcp_response", "prepare_dhcp_request", "dhcp_request_probe"] },
        "frame_hex": { "type": "string" },
        "client_mac": { "type": "string" },
        "transaction_id": { "type": "string" },
        "requested_ip": { "type": "string" },
        "server_identifier": { "type": "string" },
        "interface": { "type": "string" },
        "vlan": { "type": "integer", "minimum": 1, "maximum": 4094 }
      }
    },
    "reason": {
      "type": "array",
      "items": { "type": "string" }
    },
    "expected_observation": {
      "type": "array",
      "items": { "type": "string" }
    },
    "final_answer": { "type": ["string", "null"] }
  },
  "required": ["action_type", "objective"]
}"#;

/// Narrow constrained decoding to the tools and arguments valid for the
/// current endpoint task. A free-text graph query cannot search by MAC.
pub fn build_goal_planner_schema(registered_devices: &[String], goal: &str) -> String {
    let mut schema: serde_json::Value =
        serde_json::from_str(&build_planner_schema(registered_devices))
            .expect("planner schema is valid JSON");
    if let Some(mac) = mac_lookup_target(goal)
        .filter(|_| !crate::harness::intent::is_configuration_change_request(goal))
    {
        let tools = if requires_ping(goal) {
            vec!["find_ip_by_mac", "get_state", "self_network_ping"]
        } else {
            vec!["find_ip_by_mac", "get_state"]
        };
        schema["properties"]["tool"] = serde_json::json!({
            "anyOf": [{"type":"string","enum":tools},{"type":"null"}]
        });
        let parameters = &mut schema["properties"]["parameters"];
        if let Some(properties) = parameters["properties"].as_object_mut() {
            properties
                .retain(|key, _| ["device", "resource", "mac", "host"].contains(&key.as_str()));
            properties.insert(
                "mac".to_string(),
                serde_json::json!({"type":"string","enum":[mac]}),
            );
            properties.insert(
                "resource".to_string(),
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
    fn test_build_planner_schema_conversion() {
        let empty_schema = build_planner_schema(&[]);
        assert!(llama_cpp_2::json_schema_to_grammar(&empty_schema).is_ok());

        let devices = vec![
            "NakaokuGW".to_string(),
            "192.168.50.1".to_string(),
            "rt01".to_string(),
        ];
        let dynamic_schema = build_planner_schema(&devices);
        assert!(dynamic_schema.contains(r#""enum": ["NakaokuGW","192.168.50.1","rt01"]"#));
        let res = llama_cpp_2::json_schema_to_grammar(&dynamic_schema);
        assert!(res.is_ok(), "Schema conversion failed: {:?}", res);
    }
}
