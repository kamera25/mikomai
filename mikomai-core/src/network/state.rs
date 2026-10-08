//! Deterministic operations on stored canonical resources; no IO or inference.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_LIMIT: usize = 1000;
fn default_limit() -> usize {
    100
}
fn default_scope() -> String {
    "all".into()
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryStateInput {
    pub snapshot_id: String,
    pub device: String,
    pub resource: String,
    #[serde(default = "default_scope")]
    pub scope: String,
    #[serde(default)]
    pub filter: BTreeMap<String, Value>,
    #[serde(default)]
    pub fields: Vec<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffStateInput {
    pub before: String,
    pub after: String,
    pub device: String,
    pub resource: String,
    #[serde(default = "default_scope")]
    pub scope: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}
/// Storage-neutral snapshot of one resource and acquisition scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub snapshot_id: String,
    pub device: String,
    pub resource: String,
    pub scope: String,
    pub collected_at: String,
    pub source_id: String,
    pub normalizer_version: String,
    pub complete: bool,
    pub canonical: Option<Value>,
}
#[derive(Debug, Clone)]
pub struct ResourceShape {
    pub collection: String,
    pub identity: Vec<String>,
    pub fields: Vec<String>,
    /// Only explicitly unordered field collections are sorted.
    pub unordered_fields: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct SnapshotMetadata {
    pub snapshot_id: String,
    pub device: String,
    pub resource: String,
    pub scope: String,
    pub collected_at: String,
    pub source_id: String,
    pub normalizer_version: String,
    pub model_version: Option<String>,
    pub complete: bool,
}
impl StateSnapshot {
    pub fn metadata(&self) -> SnapshotMetadata {
        SnapshotMetadata {
            snapshot_id: self.snapshot_id.clone(),
            device: self.device.clone(),
            resource: self.resource.clone(),
            scope: self.scope.clone(),
            collected_at: self.collected_at.clone(),
            source_id: self.source_id.clone(),
            normalizer_version: self.normalizer_version.clone(),
            model_version: self
                .canonical
                .as_ref()
                .and_then(|v| v["version"].as_str())
                .map(str::to_owned),
            complete: self.complete,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct QueryStateOutput {
    pub snapshot: SnapshotMetadata,
    pub availability: String,
    pub results: Vec<Value>,
    pub matched: usize,
    pub truncated: bool,
}
#[derive(Debug, Serialize)]
pub struct StateChange {
    pub operation: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<Value>,
}
#[derive(Debug, Serialize)]
pub struct DiffStateOutput {
    pub before: SnapshotMetadata,
    pub after: SnapshotMetadata,
    pub changes: Vec<StateChange>,
    pub total_changes: usize,
    pub truncated: bool,
}
pub fn validate_selector(
    device: &str,
    resource: &str,
    scope: &str,
    ids: &[&str],
    limit: usize,
) -> Result<(), String> {
    if [device, resource, scope]
        .into_iter()
        .chain(ids.iter().copied())
        .any(|s| s.trim().is_empty() || s.len() > 256)
    {
        return Err("Invalid state selector: expected nonempty strings up to 256 bytes".into());
    }
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(format!("State limit must be 1..={MAX_LIMIT}"));
    }
    Ok(())
}
fn verify_snapshot(
    snapshot: &StateSnapshot,
    id: &str,
    device: &str,
    resource: &str,
    scope: &str,
) -> Result<(), String> {
    if snapshot.device != device
        || snapshot.resource != resource
        || snapshot.scope != scope
        || (id != "latest" && snapshot.snapshot_id != id)
    {
        return Err("State snapshot selector mismatch".into());
    }
    Ok(())
}
fn escape(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}
fn records(
    snapshot: &StateSnapshot,
    shape: &ResourceShape,
) -> Result<BTreeMap<String, Value>, String> {
    let canonical = snapshot
        .canonical
        .as_ref()
        .ok_or("State unavailable: observation has no canonical data")?;
    let rows = canonical[&shape.collection]
        .as_array()
        .ok_or("State unavailable: canonical collection is missing")?;
    let mut records = BTreeMap::new();
    for row in rows {
        let object = row.as_object().ok_or("Invalid canonical resource record")?;
        let key = shape
            .identity
            .iter()
            .map(|field| {
                object
                    .get(field)
                    .ok_or_else(|| format!("Missing resource identity: {field}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let key = serde_json::to_string(&key).map_err(|e| e.to_string())?;
        if records.insert(key, row.clone()).is_some() {
            return Err("Ambiguous duplicate canonical resource identity".into());
        }
    }
    Ok(records)
}
pub fn query_state(
    snapshot: &StateSnapshot,
    input: &QueryStateInput,
    shape: &ResourceShape,
) -> Result<QueryStateOutput, String> {
    validate_selector(
        &input.device,
        &input.resource,
        &input.scope,
        &[&input.snapshot_id],
        input.limit,
    )?;
    verify_snapshot(
        snapshot,
        &input.snapshot_id,
        &input.device,
        &input.resource,
        &input.scope,
    )?;
    if input.fields.len() > shape.fields.len()
        || input.filter.len() > shape.fields.len()
        || input
            .fields
            .iter()
            .chain(input.filter.keys())
            .any(|f| !shape.fields.contains(f))
    {
        return Err("Unknown or excessive state query fields".into());
    }
    if input
        .filter
        .values()
        .any(|v| v.is_object() || v.is_array() || v.as_str().is_some_and(|s| s.len() > 4096))
    {
        return Err("State filter requires scalar equality values up to 4096 bytes".into());
    }
    if snapshot.canonical.is_none() {
        return Ok(QueryStateOutput {
            snapshot: snapshot.metadata(),
            availability: "unavailable".into(),
            results: vec![],
            matched: 0,
            truncated: false,
        });
    }
    let rows = records(snapshot, shape)?;
    let mut results = Vec::new();
    let mut matched = 0;
    for row in rows
        .values()
        .filter(|row| input.filter.iter().all(|(k, v)| row.get(k) == Some(v)))
    {
        matched += 1;
        if results.len() < input.limit {
            results.push(if input.fields.is_empty() {
                row.clone()
            } else {
                Value::Object(
                    input
                        .fields
                        .iter()
                        .filter_map(|f| row.get(f).map(|v| (f.clone(), v.clone())))
                        .collect(),
                )
            });
        }
    }
    Ok(QueryStateOutput {
        snapshot: snapshot.metadata(),
        availability: if snapshot.complete {
            "complete"
        } else {
            "partial"
        }
        .into(),
        results,
        matched,
        truncated: matched > input.limit,
    })
}
fn equivalent(before: &Value, after: &Value, unordered: bool) -> bool {
    if unordered {
        if let (Some(a), Some(b)) = (before.as_array(), after.as_array()) {
            let sort = |v: &Vec<Value>| {
                let mut values = v.iter().map(Value::to_string).collect::<Vec<_>>();
                values.sort();
                values
            };
            return sort(a) == sort(b);
        }
    }
    before == after
}
fn walk(
    path: &str,
    before: Option<&Value>,
    after: Option<&Value>,
    changes: &mut Vec<StateChange>,
    unordered: bool,
) {
    match (before, after) {
        (Some(a), Some(b)) if equivalent(a, b, unordered) => {}
        (Some(Value::Object(a)), Some(Value::Object(b))) => {
            for key in a.keys().chain(b.keys()).collect::<BTreeSet<_>>() {
                walk(
                    &format!("{path}/{}", escape(key)),
                    a.get(key),
                    b.get(key),
                    changes,
                    false,
                );
            }
        }
        (a, b) => changes.push(StateChange {
            operation: if a.is_none() {
                "add"
            } else if b.is_none() {
                "remove"
            } else {
                "replace"
            }
            .into(),
            path: path.into(),
            before: a.cloned(),
            after: b.cloned(),
        }),
    }
}
pub fn diff_state(
    before: &StateSnapshot,
    after: &StateSnapshot,
    input: &DiffStateInput,
    shape: &ResourceShape,
) -> Result<DiffStateOutput, String> {
    validate_selector(
        &input.device,
        &input.resource,
        &input.scope,
        &[&input.before, &input.after],
        input.limit,
    )?;
    verify_snapshot(
        before,
        &input.before,
        &input.device,
        &input.resource,
        &input.scope,
    )?;
    verify_snapshot(
        after,
        &input.after,
        &input.device,
        &input.resource,
        &input.scope,
    )?;
    if !before.complete || !after.complete {
        return Err("Cannot diff incomplete state snapshots".into());
    }
    if before.device != after.device
        || before.resource != after.resource
        || before.scope != after.scope
    {
        return Err("State snapshot scope mismatch".into());
    }
    if before.metadata().model_version.is_none() || after.metadata().model_version.is_none() {
        return Err("Missing state model version".into());
    }
    if before.metadata().model_version != after.metadata().model_version
        || before.normalizer_version != after.normalizer_version
    {
        return Err("Incompatible state model/normalizer versions".into());
    }
    let a = records(before, shape)?;
    let b = records(after, shape)?;
    let mut changes = Vec::new();
    for key in a.keys().chain(b.keys()).collect::<BTreeSet<_>>() {
        let path = format!("/{}/{}", escape(&shape.collection), escape(key));
        match (a.get(key), b.get(key)) {
            (Some(Value::Object(old)), Some(Value::Object(new))) => {
                for field in old.keys().chain(new.keys()).collect::<BTreeSet<_>>() {
                    walk(
                        &format!("{path}/{}", escape(field)),
                        old.get(field),
                        new.get(field),
                        &mut changes,
                        shape.unordered_fields.contains(field),
                    );
                }
            }
            (old, new) => walk(&path, old, new, &mut changes, false),
        }
    }
    let total_changes = changes.len();
    changes.truncate(input.limit);
    Ok(DiffStateOutput {
        before: before.metadata(),
        after: after.metadata(),
        changes,
        total_changes,
        truncated: total_changes > input.limit,
    })
}
/// JSON schemas complement the typed serde input/output contracts.
pub fn input_schema(diff: bool) -> Value {
    let mut properties = json!({"device":{"type":"string","minLength":1,"maxLength":256},"resource":{"type":"string","minLength":1,"maxLength":256},"scope":{"type":"string","default":"all","minLength":1,"maxLength":256},"limit":{"type":"integer","minimum":1,"maximum":MAX_LIMIT,"default":100}});
    let required = if diff {
        properties["before"] = json!({"type":"string","minLength":1,"maxLength":256});
        properties["after"] = properties["before"].clone();
        vec!["before", "after", "device", "resource"]
    } else {
        properties["snapshot_id"] = json!({"type":"string","minLength":1,"maxLength":256});
        properties["filter"] = json!({"type":"object","additionalProperties":{"type":["string","number","boolean","null"]}});
        properties["fields"] = json!({"type":"array","items":{"type":"string"}});
        vec!["snapshot_id", "device", "resource"]
    };
    json!({"type":"object","additionalProperties":false,"required":required,"properties":properties})
}

pub fn output_schema(diff: bool) -> Value {
    let metadata = json!({"type":"object","required":["snapshot_id","device","resource","scope","collected_at","source_id","normalizer_version","model_version","complete"],"properties":{
        "snapshot_id":{"type":"string"},"device":{"type":"string"},"resource":{"type":"string"},"scope":{"type":"string"},"collected_at":{"type":"string","format":"date-time"},"source_id":{"type":"string"},"normalizer_version":{"type":"string"},"model_version":{"type":["string","null"]},"complete":{"type":"boolean"}
    }});
    if diff {
        json!({"type":"object","required":["before","after","changes","total_changes","truncated"],"properties":{
            "before":metadata,"after":metadata,"changes":{"type":"array","maxItems":MAX_LIMIT,"items":{"type":"object","required":["operation","path"],"properties":{"operation":{"type":"string","enum":["add","remove","replace"]},"path":{"type":"string"},"before":{},"after":{}}}},"total_changes":{"type":"integer","minimum":0},"truncated":{"type":"boolean"}
        }})
    } else {
        json!({"type":"object","required":["snapshot","availability","results","matched","truncated"],"properties":{
            "snapshot":metadata,"availability":{"type":"string","enum":["complete","partial","unavailable"]},"results":{"type":"array","maxItems":MAX_LIMIT,"items":{"type":"object"}},"matched":{"type":"integer","minimum":0},"truncated":{"type":"boolean"}
        }})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn shape() -> ResourceShape {
        ResourceShape {
            collection: "interfaces".into(),
            identity: vec!["name".into()],
            fields: vec![
                "name".into(),
                "status".into(),
                "prefix_len".into(),
                "ipv4_addresses".into(),
            ],
            unordered_fields: vec!["ipv4_addresses".into()],
        }
    }
    fn snapshot(rows: Value) -> StateSnapshot {
        StateSnapshot {
            snapshot_id: "v1".into(),
            device: "R1".into(),
            resource: "interfaces".into(),
            scope: "all".into(),
            collected_at: "2026-10-08T00:00:00Z".into(),
            source_id: "get_state.interfaces:all".into(),
            normalizer_version: "interface-v1".into(),
            complete: true,
            canonical: Some(json!({"version":"1.0","interfaces":rows})),
        }
    }
    fn query() -> QueryStateInput {
        serde_json::from_value(
            json!({"snapshot_id":"latest","device":"R1","resource":"interfaces"}),
        )
        .unwrap()
    }
    fn diff() -> DiffStateInput {
        serde_json::from_value(
            json!({"before":"v1","after":"latest","device":"R1","resource":"interfaces"}),
        )
        .unwrap()
    }
    #[test]
    fn query_filter_projection_and_no_match() {
        let snap = snapshot(json!([{"name":"eth2","status":"down"},{"name":"eth1","status":"up"}]));
        let mut input = query();
        input.filter.insert("name".into(), json!("eth1"));
        input.fields = vec!["status".into()];
        let result = query_state(&snap, &input, &shape()).unwrap();
        assert_eq!(result.results, vec![json!({"status":"up"})]);
        assert_eq!(result.snapshot.snapshot_id, "v1");
        input.filter.insert("name".into(), json!("missing"));
        assert!(query_state(&snap, &input, &shape())
            .unwrap()
            .results
            .is_empty());
    }
    #[test]
    fn query_limits_and_validation() {
        let snap = snapshot(json!([{"name":"eth2"},{"name":"eth1"}]));
        let mut input = query();
        input.limit = 1;
        let result = query_state(&snap, &input, &shape()).unwrap();
        assert_eq!(result.results, vec![json!({"name":"eth1"})]);
        assert_eq!(result.matched, 2);
        assert!(result.truncated);
        for limit in [0, 1001] {
            input.limit = limit;
            assert!(query_state(&snap, &input, &shape()).is_err());
        }
        input.limit = 100;
        input.fields = vec!["typo".into()];
        assert!(query_state(&snap, &input, &shape()).is_err());
        input.fields.clear();
        input.filter.insert("name".into(), json!({"eq":"eth1"}));
        assert!(query_state(&snap, &input, &shape()).is_err());
    }
    #[test]
    fn incomplete_query_distinguishes_unavailable_and_partial() {
        let mut snap = snapshot(json!([{"name":"eth1"}]));
        snap.complete = false;
        assert_eq!(
            query_state(&snap, &query(), &shape()).unwrap().availability,
            "partial"
        );
        snap.canonical = None;
        let result = query_state(&snap, &query(), &shape()).unwrap();
        assert_eq!(result.availability, "unavailable");
        assert!(!result.snapshot.complete);
    }
    #[test]
    fn no_diff_for_collection_and_address_reordering() {
        let a = snapshot(
            json!([{"name":"eth1","ipv4_addresses":["192.0.2.1","192.0.2.2"]},{"name":"eth2"}]),
        );
        let b = snapshot(
            json!([{"name":"eth2"},{"name":"eth1","ipv4_addresses":["192.0.2.2","192.0.2.1"]}]),
        );
        assert!(diff_state(&a, &b, &diff(), &shape())
            .unwrap()
            .changes
            .is_empty());
    }
    #[test]
    fn additions_removals_and_multiple_field_changes() {
        let a = snapshot(json!([{"name":"eth1","status":"up","prefix_len":24},{"name":"eth2"}]));
        let b = snapshot(json!([{"name":"eth1","status":"down","prefix_len":16},{"name":"eth3"}]));
        let changes = diff_state(&a, &b, &diff(), &shape()).unwrap().changes;
        assert_eq!(changes.len(), 4);
        assert_eq!(changes[0].path, "/interfaces/[\"eth1\"]/prefix_len");
        assert_eq!(changes[1].before, Some(json!("up")));
        assert_eq!(changes[1].after, Some(json!("down")));
        assert_eq!(changes[2].operation, "remove");
        assert_eq!(changes[3].operation, "add");
    }
    #[test]
    fn null_missing_unknown_and_path_escaping() {
        let a = snapshot(json!([{"name":"et~/1","prefix_len":null}]));
        let b = snapshot(json!([{"name":"et~/1","status":"unknown"}]));
        let output = diff_state(&a, &b, &diff(), &shape()).unwrap();
        assert_eq!(output.changes[0].before, Some(Value::Null));
        assert_eq!(output.changes[0].after, None);
        assert!(output.changes[0].path.contains("et~0~11"));
        let json = serde_json::to_value(&output).unwrap();
        assert!(json["changes"][0].get("before").unwrap().is_null());
        assert!(json["changes"][0].get("after").is_none());
    }
    #[test]
    fn incomplete_incompatible_and_invalid_diff() {
        let a = snapshot(json!([]));
        let mut b = a.clone();
        b.complete = false;
        assert!(diff_state(&a, &b, &diff(), &shape()).is_err());
        b = a.clone();
        b.canonical.as_mut().unwrap()["version"] = json!("2.0");
        assert!(diff_state(&a, &b, &diff(), &shape()).is_err());
        b = a.clone();
        b.scope = "eth1".into();
        assert!(diff_state(&a, &b, &diff(), &shape()).is_err());
        let mut input = diff();
        input.before = "".into();
        assert!(diff_state(&a, &a, &input, &shape()).is_err());
    }
    #[test]
    fn diff_limit_and_duplicate_identities() {
        let a = snapshot(json!([]));
        let b = snapshot(json!([{"name":"eth1"},{"name":"eth2"}]));
        let mut input = diff();
        input.limit = 1;
        let result = diff_state(&a, &b, &input, &shape()).unwrap();
        assert!(result.truncated);
        assert_eq!(result.total_changes, 2);
        assert_eq!(result.changes.len(), 1);
        let duplicate = snapshot(json!([{"name":"eth1"},{"name":"eth1"}]));
        assert!(diff_state(&a, &duplicate, &diff(), &shape()).is_err());
    }
}
