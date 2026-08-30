#![allow(dead_code)]

use serde::Deserialize;
use serde_yaml::Value;
use std::collections::HashMap;

/// Schema definitions for parsing the rule YAML (YANG-like constraints)
#[derive(Debug, Deserialize, Clone)]
pub struct SchemaRoot {
    pub schema: HashMap<String, SchemaNode>,
}

/// A node in the schema representing type, properties, and constraints
#[derive(Debug, Deserialize, Clone)]
pub struct SchemaNode {
    #[serde(rename = "type")]
    pub node_type: String,

    #[serde(default)]
    pub properties: Option<HashMap<String, SchemaNode>>,

    #[serde(default)]
    pub required: bool,

    #[serde(default)]
    pub range: Option<String>,

    #[serde(rename = "enum", default)]
    pub enum_values: Option<Vec<String>>,
}

/// A validation error detailing the path and what failed
#[derive(Debug, PartialEq)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

/// The validation engine that loads a schema and checks YAML input (AST) against it
pub struct AstValidator {
    schema: SchemaRoot,
}

impl AstValidator {
    /// Initialize the validator with a schema YAML string
    pub fn new(schema_yaml: &str) -> Result<Self, String> {
        let schema: SchemaRoot = serde_yaml::from_str(schema_yaml)
            .map_err(|e| format!("Failed to parse schema: {}", e))?;
        Ok(Self { schema })
    }

    /// Validate the given LLM-generated YAML input string against the schema
    pub fn validate(&self, input_yaml: &str) -> Result<Vec<ValidationError>, String> {
        let input: Value = serde_yaml::from_str(input_yaml)
            .map_err(|e| format!("Failed to parse input YAML: {}", e))?;

        let mut errors = Vec::new();

        if let Value::Mapping(root_map) = input {
            // Check for unknown keys and validate known ones at the root
            for (key, val) in &root_map {
                if let Value::String(k) = key {
                    if let Some(schema_node) = self.schema.schema.get(k) {
                        self.validate_node(k, val, schema_node, &mut errors);
                    } else {
                        errors.push(ValidationError {
                            path: k.clone(),
                            message: format!("Unknown property: '{}'", k),
                        });
                    }
                }
            }

            // Check for missing required properties at the root
            for (k, schema_node) in &self.schema.schema {
                if schema_node.required {
                    let key_val = Value::String(k.clone());
                    if !root_map.contains_key(&key_val) {
                        errors.push(ValidationError {
                            path: k.clone(),
                            message: format!("Missing required property: '{}'", k),
                        });
                    }
                }
            }
        } else {
            return Err("Input YAML root must be a container (mapping)".to_string());
        }

        Ok(errors)
    }

    /// Recursively traverse the AST and validate nodes according to the schema
    fn validate_node(
        &self,
        path: &str,
        value: &Value,
        schema_node: &SchemaNode,
        errors: &mut Vec<ValidationError>,
    ) {
        match schema_node.node_type.as_str() {
            "container" => {
                if let Value::Mapping(map) = value {
                    if let Some(props) = &schema_node.properties {
                        // Validate child nodes and check for unknown properties
                        for (k, v) in map {
                            if let Value::String(k_str) = k {
                                let child_path = format!("{}.{}", path, k_str);
                                if let Some(child_schema) = props.get(k_str) {
                                    self.validate_node(&child_path, v, child_schema, errors);
                                } else {
                                    errors.push(ValidationError {
                                        path: child_path,
                                        message: format!("Unknown property: '{}'", k_str),
                                    });
                                }
                            }
                        }

                        // Check for missing required properties inside the container
                        for (k, child_schema) in props {
                            if child_schema.required {
                                let key_val = Value::String(k.clone());
                                if !map.contains_key(&key_val) {
                                    errors.push(ValidationError {
                                        path: format!("{}.{}", path, k),
                                        message: format!("Missing required property: '{}'", k),
                                    });
                                }
                            }
                        }
                    }
                } else {
                    errors.push(ValidationError {
                        path: path.to_string(),
                        message: format!(
                            "Type mismatch: expected container, got {}",
                            Self::get_type_name(value)
                        ),
                    });
                }
            }
            "integer" => {
                if let Value::Number(num) = value {
                    if let Some(i) = num.as_i64() {
                        if let Some(range_str) = &schema_node.range {
                            if let Some((min, max)) = Self::parse_range(range_str) {
                                if i < min || i > max {
                                    errors.push(ValidationError {
                                        path: path.to_string(),
                                        message: format!(
                                            "Value {} is out of range {}",
                                            i, range_str
                                        ),
                                    });
                                }
                            } else {
                                // Invalid range schema definition
                                errors.push(ValidationError {
                                    path: path.to_string(),
                                    message: format!(
                                        "Invalid range definition in schema: '{}'",
                                        range_str
                                    ),
                                });
                            }
                        }
                    } else {
                        errors.push(ValidationError {
                            path: path.to_string(),
                            message: "Type mismatch: expected integer".to_string(),
                        });
                    }
                } else {
                    errors.push(ValidationError {
                        path: path.to_string(),
                        message: format!(
                            "Type mismatch: expected integer, got {}",
                            Self::get_type_name(value)
                        ),
                    });
                }
            }
            "string" => {
                if let Value::String(s) = value {
                    if let Some(enum_vals) = &schema_node.enum_values {
                        if !enum_vals.contains(s) {
                            errors.push(ValidationError {
                                path: path.to_string(),
                                message: format!(
                                    "Value '{}' is not in allowed enum {:?}",
                                    s, enum_vals
                                ),
                            });
                        }
                    }
                } else {
                    errors.push(ValidationError {
                        path: path.to_string(),
                        message: format!(
                            "Type mismatch: expected string, got {}",
                            Self::get_type_name(value)
                        ),
                    });
                }
            }
            unknown_type => {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!("Unsupported schema type: {}", unknown_type),
                });
            }
        }
    }

    /// Helper to parse a range string like "1-4094" into a min/max tuple
    fn parse_range(range_str: &str) -> Option<(i64, i64)> {
        let parts: Vec<&str> = range_str.split('-').collect();
        if parts.len() == 2 {
            if let (Ok(min), Ok(max)) = (
                parts[0].trim().parse::<i64>(),
                parts[1].trim().parse::<i64>(),
            ) {
                return Some((min, max));
            }
        }
        None
    }

    /// Helper to determine the string representation of a YAML value's type
    fn get_type_name(value: &Value) -> &'static str {
        match value {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Sequence(_) => "list",
            Value::Mapping(_) => "container",
            Value::Tagged(_) => "tagged",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEMA_YAML: &str = r#"
schema:
  vlan_config:
    type: "container"
    properties:
      vlan-id:
        type: "integer"
        range: "1-4094"
        required: true
      name:
        type: "string"
        required: false
      interface-mode:
        type: "string"
        enum: ["ACCESS", "TRUNK"]
        required: true
"#;

    #[test]
    fn test_valid_yaml() {
        let input_yaml = r#"
vlan_config:
  vlan-id: 10
  name: "Sales"
  interface-mode: "ACCESS"
"#;
        let validator = AstValidator::new(SCHEMA_YAML).expect("Valid schema");
        let errors = validator.validate(input_yaml).expect("Valid input parse");
        assert!(
            errors.is_empty(),
            "Expected no errors for valid YAML, got {:?}",
            errors
        );
    }

    #[test]
    fn test_invalid_yaml() {
        let input_yaml = r#"
vlan_config:
  vlan-id: 5000
  interface-mode: "Hybrid"
  status: "active"
"#;
        let validator = AstValidator::new(SCHEMA_YAML).expect("Valid schema");
        let errors = validator.validate(input_yaml).expect("Valid input parse");

        assert_eq!(errors.len(), 3, "Expected 3 validation errors");

        // Verify out of range error
        assert!(errors
            .iter()
            .any(|e| e.path == "vlan_config.vlan-id" && e.message.contains("out of range")));

        // Verify enum error
        assert!(errors
            .iter()
            .any(|e| e.path == "vlan_config.interface-mode"
                && e.message.contains("not in allowed enum")));

        // Verify unknown property error
        assert!(errors
            .iter()
            .any(|e| e.path == "vlan_config.status" && e.message.contains("Unknown property")));
    }

    #[test]
    fn test_missing_required() {
        let input_yaml = r#"
vlan_config:
  name: "Marketing"
"#;
        let validator = AstValidator::new(SCHEMA_YAML).expect("Valid schema");
        let errors = validator.validate(input_yaml).expect("Valid input parse");

        assert_eq!(
            errors.len(),
            2,
            "Expected 2 validation errors (vlan-id and interface-mode missing)"
        );

        assert!(errors
            .iter()
            .any(|e| e.path == "vlan_config.vlan-id"
                && e.message.contains("Missing required property")));
        assert!(errors.iter().any(|e| e.path == "vlan_config.interface-mode"
            && e.message.contains("Missing required property")));
    }
}
