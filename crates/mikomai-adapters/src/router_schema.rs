//! Native router graph tables. OpenConfig is a design reference, not a node kind.
use serde::Deserialize;
use serde_json::Value;
use std::sync::LazyLock;

pub const ROUTER_SCHEMA_SQL: &str = include_str!("schema/router-schema.surql");

#[derive(Debug, Deserialize)]
pub struct RouterField {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub required: bool,
    pub minimum: Option<i64>,
    pub maximum: Option<i64>,
    #[serde(default)]
    pub allowed: Vec<Value>,
}

#[derive(Debug, Deserialize)]
pub struct RouterResource {
    pub table: String,
    pub identity: Vec<String>,
    pub fields: Vec<RouterField>,
}

pub static ROUTER_RESOURCES: LazyLock<Vec<RouterResource>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("schema/router-resources.json"))
        .expect("bundled native router schema catalog must be valid")
});

pub fn resource_schema(table: &str) -> Result<&'static RouterResource, String> {
    ROUTER_RESOURCES
        .iter()
        .find(|resource| resource.table == table)
        .ok_or_else(|| format!("Unsupported router graph table: {table}"))
}

/// Validate native normalized records before observation persistence. Ranges
/// and uniqueness are also enforced by SurrealDB when writing the typed rows.
pub(crate) fn validate_normalized(value: &Value) -> Result<(), String> {
    for resource in ROUTER_RESOURCES.iter() {
        let Some(records) = value.get(&resource.table) else {
            continue;
        };
        let records = records
            .as_array()
            .ok_or_else(|| format!("{} must be an array", resource.table))?;
        let mut identities = std::collections::HashSet::new();
        for record in records {
            let object = record
                .as_object()
                .ok_or("Router graph record must be an object")?;
            for name in object.keys() {
                if !resource.fields.iter().any(|field| field.name == *name) {
                    return Err(format!("Unknown {} field: {name}", resource.table));
                }
            }
            for field in &resource.fields {
                let Some(value) = record.get(&field.name) else {
                    if field.required {
                        return Err(format!("{}.{} is required", resource.table, field.name));
                    }
                    continue;
                };
                let valid = match field.field_type.as_str() {
                    "string" => value
                        .as_str()
                        .is_some_and(|v| !field.required || !v.trim().is_empty()),
                    "int" => value.as_i64().is_some_and(|v| v >= 0),
                    "float" => value.is_number(),
                    "bool" => value.is_boolean(),
                    "object" => value.is_object(),
                    "array<object>" => value
                        .as_array()
                        .is_some_and(|v| v.iter().all(Value::is_object)),
                    "array<string>" => value
                        .as_array()
                        .is_some_and(|v| v.iter().all(Value::is_string)),
                    "array<int>" => value
                        .as_array()
                        .is_some_and(|v| v.iter().all(|n| n.as_i64().is_some_and(|n| n >= 0))),
                    _ => false,
                };
                if !valid {
                    return Err(format!(
                        "Invalid type for {}.{}: expected {}",
                        resource.table, field.name, field.field_type
                    ));
                }
                if !field.allowed.is_empty() && !field.allowed.contains(value) {
                    return Err(format!(
                        "Invalid value for {}.{}",
                        resource.table, field.name
                    ));
                }
                if let Some(number) = value.as_i64() {
                    if field.minimum.is_some_and(|min| number < min)
                        || field.maximum.is_some_and(|max| number > max)
                    {
                        return Err(format!("Out of range: {}.{}", resource.table, field.name));
                    }
                }
                // OpenConfig SPIs are uint64. Store decimal strings to retain
                // values beyond SurrealDB's signed integer range without loss.
                if resource.table == "ike_sa"
                    && matches!(field.name.as_str(), "initiator_spi" | "responder_spi")
                {
                    let text = value.as_str().ok_or("SPI must be a decimal string")?;
                    if text.parse::<u64>().is_err()
                        || text.starts_with('+')
                        || (text.len() > 1 && text.starts_with('0'))
                    {
                        return Err("SPI must be a canonical decimal uint64 string".into());
                    }
                }
            }
            let identity: Vec<_> = resource.identity.iter().map(|key| &record[key]).collect();
            if !identities.insert(serde_json::to_string(&identity).map_err(|e| e.to_string())?) {
                return Err(format!("Duplicate {} identity", resource.table));
            }
        }
    }
    Ok(())
}
