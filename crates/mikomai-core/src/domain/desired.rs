//! Declarative, vendor-independent desired-state patches.
//!
//! This first version supports interface properties only. Applying a patch is a
//! pure operation: it never changes observations, persists data, or runs commands.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Interface,
}

/// Device-scoped identity; names containing dots or slashes remain unambiguous.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityRef {
    pub entity_type: EntityType,
    pub device: String,
    pub id: String,
}

impl EntityRef {
    pub fn interface(device: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            entity_type: EntityType::Interface,
            device: device.into(),
            id: id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateEntity {
    pub target: EntityRef,
    pub properties: BTreeMap<String, Value>,
}

/// Detached canonical snapshot used as either Current or Desired.
/// Unchanged properties and opaque relationships are preserved verbatim.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateGraph {
    pub entities: Vec<StateEntity>,
    #[serde(default)]
    pub relationships: Vec<Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesiredStatePatch {
    pub mutations: Vec<Mutation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mutation {
    SetProperty {
        target: EntityRef,
        property: String,
        value: Value,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyChange {
    pub target: EntityRef,
    pub property: String,
    /// None means the property was not present in Current.
    /// JSON consumers should treat both absence and null as unknown prior state.
    pub before: Option<Value>,
    pub after: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum PatchError {
    InvalidTarget {
        target: EntityRef,
    },
    DuplicateEntity {
        target: EntityRef,
    },
    TargetNotFound {
        target: EntityRef,
    },
    UnsupportedProperty {
        target: EntityRef,
        property: String,
    },
    InvalidValue {
        target: EntityRef,
        property: String,
        expected: String,
    },
    DuplicateMutation {
        target: EntityRef,
        property: String,
    },
}

impl std::fmt::Display for PatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "desired-state patch validation failed: {self:?}")
    }
}
impl std::error::Error for PatchError {}

fn validate_target(target: &EntityRef) -> Result<(), PatchError> {
    if target.device.trim().is_empty() || target.id.trim().is_empty() {
        return Err(PatchError::InvalidTarget {
            target: target.clone(),
        });
    }
    Ok(())
}

impl DesiredStatePatch {
    /// Validate all mutations before applying any. Repeated writes to the same
    /// property are rejected, so mutation order cannot resolve conflicting intent.
    pub fn validate(&self, current: &StateGraph) -> Result<(), PatchError> {
        let mut entities = BTreeSet::new();
        for entity in &current.entities {
            validate_target(&entity.target)?;
            if !entities.insert(&entity.target) {
                return Err(PatchError::DuplicateEntity {
                    target: entity.target.clone(),
                });
            }
        }
        let mut writes = BTreeSet::new();
        for mutation in &self.mutations {
            let Mutation::SetProperty {
                target,
                property,
                value,
            } = mutation;
            validate_target(target)?;
            if !entities.contains(target) {
                return Err(PatchError::TargetNotFound {
                    target: target.clone(),
                });
            }
            if !writes.insert((target, property)) {
                return Err(PatchError::DuplicateMutation {
                    target: target.clone(),
                    property: property.clone(),
                });
            }
            let (valid, expected) = match property.as_str() {
                "admin_state" => (matches!(value.as_str(), Some("up" | "down")), "up or down"),
                "mtu" => (
                    value.as_u64().is_some_and(|v| (576..=9216).contains(&v)),
                    "integer in 576..=9216",
                ),
                "description" => (value.is_string(), "string"),
                _ => {
                    return Err(PatchError::UnsupportedProperty {
                        target: target.clone(),
                        property: property.clone(),
                    })
                }
            };
            if !valid {
                return Err(PatchError::InvalidValue {
                    target: target.clone(),
                    property: property.clone(),
                    expected: expected.into(),
                });
            }
        }
        Ok(())
    }

    /// Current + Patch = Desired. Current is unchanged even if validation fails.
    pub fn apply(&self, current: &StateGraph) -> Result<StateGraph, PatchError> {
        self.validate(current)?;
        let mut desired = current.clone();
        let mut entities: BTreeMap<_, _> = desired
            .entities
            .iter_mut()
            .map(|entity| (entity.target.clone(), &mut entity.properties))
            .collect();
        for mutation in &self.mutations {
            let Mutation::SetProperty {
                target,
                property,
                value,
            } = mutation;
            entities
                .get_mut(target)
                .expect("validated target")
                .insert(property.clone(), value.clone());
        }
        Ok(desired)
    }

    /// Effective property differences in identity/property order, excluding no-ops.
    /// This is data for a future execution planner, not executable instructions.
    pub fn diff(&self, current: &StateGraph) -> Result<Vec<PropertyChange>, PatchError> {
        let desired = self.apply(current)?;
        let mut changes = Vec::new();
        for (before, after) in current.entities.iter().zip(&desired.entities) {
            for (property, value) in &after.properties {
                if before.properties.get(property) != Some(value) {
                    changes.push(PropertyChange {
                        target: after.target.clone(),
                        property: property.clone(),
                        before: before.properties.get(property).cloned(),
                        after: value.clone(),
                    });
                }
            }
        }
        changes.sort_by(|a, b| (&a.target, &a.property).cmp(&(&b.target, &b.property)));
        Ok(changes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn current() -> StateGraph {
        serde_json::from_value(json!({"entities":[
            {"target":{"entity_type":"interface","device":"gw","id":"eth1"},"properties":{"admin_state":"down","mtu":1500,"ip":"10.0.0.1/24"}},
            {"target":{"entity_type":"interface","device":"other","id":"eth1"},"properties":{"admin_state":"down"}}
        ],"relationships":[{"from":"gw.eth1","relation":"belongs_to","to":"blue"}]})).unwrap()
    }

    fn patch(property: &str, value: Value) -> DesiredStatePatch {
        DesiredStatePatch {
            mutations: vec![Mutation::SetProperty {
                target: EntityRef::interface("gw", "eth1"),
                property: property.into(),
                value,
            }],
        }
    }

    #[test]
    fn apply_preserves_current_and_untouched_graph_and_reports_diff() {
        let current = current();
        let original = current.clone();
        let patch = patch("admin_state", json!("up"));
        let wire = serde_json::to_string(&patch).unwrap();
        let patch: DesiredStatePatch = serde_json::from_str(&wire).unwrap();
        let desired = patch.apply(&current).unwrap();
        let mut expected = original.clone();
        expected.entities[0]
            .properties
            .insert("admin_state".into(), json!("up"));
        assert_eq!(desired, expected);
        assert_eq!(current, original);
        assert_eq!(patch.apply(&current).unwrap(), desired);
        assert_eq!(patch.apply(&desired).unwrap(), desired);
        assert!(patch.diff(&desired).unwrap().is_empty());
        assert_eq!(
            patch.diff(&current).unwrap(),
            vec![PropertyChange {
                target: EntityRef::interface("gw", "eth1"),
                property: "admin_state".into(),
                before: Some(json!("down")),
                after: json!("up")
            }]
        );
    }

    #[test]
    fn invalid_values_and_unsupported_properties_are_rejected() {
        for (key, value) in [
            ("admin_state", json!("enabled-ish")),
            ("admin_state", Value::Null),
            ("mtu", json!(575)),
            ("mtu", json!(9217)),
            ("mtu", json!(1500.5)),
            ("mtu", json!("1500")),
            ("description", json!(false)),
        ] {
            assert!(matches!(
                patch(key, value).apply(&current()),
                Err(PatchError::InvalidValue { .. })
            ));
        }
        assert!(matches!(
            patch("command", json!("no shutdown")).apply(&current()),
            Err(PatchError::UnsupportedProperty { .. })
        ));
        for (key, value) in [
            ("mtu", json!(576)),
            ("mtu", json!(9216)),
            ("description", json!("")),
        ] {
            assert!(patch(key, value).apply(&current()).is_ok());
        }
    }

    #[test]
    fn failures_are_atomic_and_targets_are_unambiguous() {
        let mut current = current();
        let original = current.clone();
        let mut invalid = patch("admin_state", json!("up"));
        invalid.mutations.extend(patch("mtu", json!(1)).mutations);
        assert!(invalid.apply(&current).is_err());
        assert_eq!(current, original);
        let mut duplicate = patch("admin_state", json!("up"));
        duplicate
            .mutations
            .extend(patch("admin_state", json!("down")).mutations);
        assert!(matches!(
            duplicate.apply(&current),
            Err(PatchError::DuplicateMutation { .. })
        ));
        current.entities.remove(0);
        assert!(matches!(
            patch("admin_state", json!("up")).apply(&current),
            Err(PatchError::TargetNotFound { .. })
        ));
        current.entities.push(current.entities[0].clone());
        assert!(matches!(
            DesiredStatePatch::default().apply(&current),
            Err(PatchError::DuplicateEntity { .. })
        ));
    }

    #[test]
    fn independent_updates_are_order_independent_and_can_add_missing_properties() {
        let current = current();
        let mut patch = patch("description", json!("uplink"));
        patch
            .mutations
            .extend(super::tests::patch("mtu", json!(9000)).mutations);
        let desired = patch.apply(&current).unwrap();
        let diff = patch.diff(&current).unwrap();
        assert_eq!(
            desired.entities[0].properties["description"],
            json!("uplink")
        );
        assert_eq!(desired.entities[0].properties["mtu"], json!(9000));
        assert_eq!(diff.len(), 2);
        assert_eq!(diff[0].property, "description");
        assert_eq!(diff[0].before, None);
        patch.mutations.reverse();
        assert_eq!(patch.apply(&current).unwrap(), desired);
        assert_eq!(patch.diff(&current).unwrap(), diff);
    }

    #[test]
    fn strict_wire_format_and_empty_patch() {
        assert_eq!(
            DesiredStatePatch::default().apply(&current()).unwrap(),
            current()
        );
        for value in [
            json!({}),
            json!({"mutations":[],"commit":true}),
            json!({"mutations":[{"op":"run_command","command":"no shutdown"}]}),
        ] {
            assert!(serde_json::from_value::<DesiredStatePatch>(value).is_err());
        }
        let mut value = serde_json::to_value(patch("admin_state", json!("up"))).unwrap();
        value["mutations"][0]["command"] = json!("no shutdown");
        assert!(serde_json::from_value::<DesiredStatePatch>(value).is_err());
        let mut invalid = patch("admin_state", json!("up"));
        let Mutation::SetProperty { target, .. } = &mut invalid.mutations[0];
        target.id = " ".into();
        assert!(matches!(
            invalid.validate(&current()),
            Err(PatchError::InvalidTarget { .. })
        ));
    }
}
