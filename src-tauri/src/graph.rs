//! Local-first network inventory backed by an embedded SurrealDB instance.
//!
//! This module is intentionally the only writer for graph data.  MCP adapters
//! supply a `GraphIngestInput`; raw payloads and provenance are retained even
//! when a normalizer cannot extract every vendor-specific setting.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use surrealdb::engine::local::{Db, RocksDb};
use surrealdb::Surreal;
use tauri::Manager;

pub const GRAPH_TTL_MINUTES: i64 = 20;

#[derive(Clone)]
pub struct SurrealDbState {
    db: Surreal<Db>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphDataKind {
    Config,
    Routing,
    Arp,
}

impl GraphDataKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Routing => "routing",
            Self::Arp => "arp",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphIngestInput {
    pub source_id: String,
    pub collected_at: DateTime<Utc>,
    pub device_name: String,
    pub kind: GraphDataKind,
    pub raw: String,
    pub normalized: Option<Value>,
    pub normalizer_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQuery {
    pub query: String,
    pub device_name: Option<String>,
    pub ip_address: Option<String>,
    pub vlan: Option<u32>,
    pub acl: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQueryResult {
    pub fresh: bool,
    pub requires_refresh: bool,
    pub facts: Vec<Value>,
    pub relationships: Vec<Value>,
    pub citations: Vec<Value>,
}

impl SurrealDbState {
    pub async fn initialize(app: &tauri::AppHandle) -> Result<Self, String> {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to resolve app data directory: {e}"))?
            .join("surrealdb");
        Self::initialize_at(&dir).await
    }

    pub async fn initialize_at(path: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create SurrealDB directory: {e}"))?;
        let db = Surreal::new::<RocksDb>(path)
            .await
            .map_err(|e| format!("Failed to open embedded SurrealDB: {e}"))?;
        db.use_ns("mikomai")
            .use_db("network_graph")
            .await
            .map_err(|e| format!("Failed to select SurrealDB namespace: {e}"))?;
        let state = Self { db };
        state.define_schema().await?;
        Ok(state)
    }

    async fn define_schema(&self) -> Result<(), String> {
        // Schema-less records retain vendor-specific data while the indexes
        // establish stable identities for the normalized network model.
        self.db
            .query(
                r#"
DEFINE TABLE device SCHEMALESS;
DEFINE TABLE interface SCHEMALESS;
DEFINE TABLE ip_address SCHEMALESS;
DEFINE TABLE subnet SCHEMALESS;
DEFINE TABLE vlan SCHEMALESS;
DEFINE TABLE route SCHEMALESS;
DEFINE TABLE acl SCHEMALESS;
DEFINE TABLE ntp_server SCHEMALESS;
DEFINE TABLE ntp_status SCHEMALESS;
DEFINE TABLE graph_edge SCHEMALESS;
DEFINE TABLE observation SCHEMALESS;
DEFINE TABLE config_snapshot SCHEMALESS;
DEFINE TABLE config_change SCHEMALESS;
DEFINE TABLE conflict SCHEMALESS;
DEFINE INDEX device_key ON TABLE device FIELDS key UNIQUE;
DEFINE INDEX observation_device_time ON TABLE observation FIELDS device_name, collected_at;
DEFINE INDEX snapshot_device_time ON TABLE config_snapshot FIELDS device_name, collected_at;
DEFINE INDEX edge_key ON TABLE graph_edge FIELDS key UNIQUE;
"#,
            )
            .await
            .map_err(|e| format!("Failed to define graph schema: {e}"))?;
        Ok(())
    }

    async fn upsert(&self, table: &str, id: &str, record: Value) -> Result<(), String> {
        let sql = format!("UPSERT type::record('{table}', $id) CONTENT $record;");
        self.db
            .query(sql)
            .bind(("id", id.to_owned()))
            .bind(("record", record))
            .await
            .map_err(|e| format!("Failed to write {table}: {e}"))?;
        Ok(())
    }

    async fn edge(
        &self,
        kind: &str,
        from: &str,
        to: &str,
        observation_id: &str,
    ) -> Result<(), String> {
        let key = format!("{}:{}:{}", kind, from, to);
        self.upsert(
            "graph_edge",
            &record_key(&key),
            json!({
                "key": key,
                "kind": kind,
                "from": from,
                "to": to,
                "observation_id": observation_id,
                "updated_at": Utc::now(),
            }),
        )
        .await
    }

    pub async fn ingest(&self, input: GraphIngestInput) -> Result<(), String> {
        if input.device_name.trim().is_empty() || input.source_id.trim().is_empty() {
            return Err("Graph ingestion requires a device name and source ID".to_string());
        }
        let observation_id = uuid::Uuid::new_v4().to_string();
        let kind = serde_json::to_string(&input.kind).unwrap_or_else(|_| "\"unknown\"".to_string());
        self.upsert(
            "observation",
            &observation_id,
            json!({
                "id": observation_id,
                "source_id": input.source_id,
                "device_name": input.device_name,
                "kind": kind.trim_matches('"'),
                "collected_at": input.collected_at,
                "raw": input.raw,
                "normalized": input.normalized,
                "normalizer_version": input.normalizer_version,
            }),
        )
        .await?;

        let device_key = record_key(&input.device_name);
        self.upsert(
            "device",
            &device_key,
            json!({
                "key": device_key,
                "name": input.device_name,
                "observation_id": observation_id,
                "observed_at": input.collected_at,
            }),
        )
        .await?;

        if matches!(&input.kind, GraphDataKind::Config) {
            self.store_config_history(&input, &observation_id).await?;
        }
        if let Some(normalized) = &input.normalized {
            self.store_normalized(
                &input.device_name,
                normalized,
                &observation_id,
                input.collected_at,
            )
            .await?;
        }
        Ok(())
    }

    async fn store_config_history(
        &self,
        input: &GraphIngestInput,
        observation_id: &str,
    ) -> Result<(), String> {
        let hash = fnv1a(&input.raw).to_string();
        let mut response = self.db.query(
            "SELECT hash, raw FROM config_snapshot WHERE device_name = $device ORDER BY collected_at DESC LIMIT 1;"
        ).bind(("device", input.device_name.clone())).await
            .map_err(|e| format!("Failed to read config history: {e}"))?;
        let previous: Vec<Value> = response
            .take(0)
            .map_err(|e| format!("Failed to decode config history: {e}"))?;
        if previous
            .first()
            .and_then(|v| v.get("hash"))
            .and_then(Value::as_str)
            == Some(hash.as_str())
        {
            return Ok(());
        }
        let snapshot_id = uuid::Uuid::new_v4().to_string();
        self.upsert(
            "config_snapshot",
            &snapshot_id,
            json!({
                "device_name": input.device_name,
                "hash": hash,
                "raw": input.raw,
                "collected_at": input.collected_at,
                "observation_id": observation_id,
            }),
        )
        .await?;
        if let Some(old) = previous.first() {
            self.upsert(
                "config_change",
                &uuid::Uuid::new_v4().to_string(),
                json!({
                    "device_name": input.device_name,
                    "before_hash": old.get("hash"),
                    "after_hash": hash,
                    "changed_at": input.collected_at,
                    "observation_id": observation_id,
                    "summary": "configuration content changed",
                }),
            )
            .await?;
        }
        Ok(())
    }

    async fn store_normalized(
        &self,
        device: &str,
        value: &Value,
        observation_id: &str,
        observed_at: DateTime<Utc>,
    ) -> Result<(), String> {
        let device_key = record_key(device);
        for interface in array(value, "interfaces") {
            let name = interface
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let key = format!("{}:{}", device, name);
            let id = record_key(&key);
            self.upsert("interface", &id, json!({"key":key,"device_name":device,"name":name,"data":interface,"observation_id":observation_id,"observed_at":observed_at})).await?;
            self.edge("has_interface", &device_key, &id, observation_id)
                .await?;
        }
        for ip in array(value, "ip_addresses") {
            let address = ip
                .get("address")
                .and_then(Value::as_str)
                .or_else(|| ip.as_str())
                .unwrap_or("");
            if address.is_empty() {
                continue;
            }
            let id = record_key(address);
            self.upsert("ip_address", &id, json!({"key":address,"address":address,"data":ip,"observation_id":observation_id,"observed_at":observed_at})).await?;
            self.edge("device_has_ip", &device_key, &id, observation_id)
                .await?;
            if let Some(subnet) = ip.get("subnet").and_then(Value::as_str) {
                let subnet_id = record_key(subnet);
                self.upsert("subnet", &subnet_id, json!({"key":subnet,"cidr":subnet,"observation_id":observation_id,"observed_at":observed_at})).await?;
                self.edge("ip_in_subnet", &id, &subnet_id, observation_id)
                    .await?;
            }
        }
        for vlan in array(value, "vlans") {
            let id_value = vlan
                .get("id")
                .and_then(Value::as_u64)
                .map(|v| v.to_string())
                .or_else(|| vlan.as_u64().map(|v| v.to_string()));
            let Some(id_value) = id_value else {
                continue;
            };
            let id = record_key(&id_value);
            self.upsert("vlan", &id, json!({"key":id_value,"data":vlan,"observation_id":observation_id,"observed_at":observed_at})).await?;
            self.edge("device_has_vlan", &device_key, &id, observation_id)
                .await?;
        }
        for route in array(value, "routes") {
            let destination = route
                .get("destination")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let gateway = route.get("gateway").and_then(Value::as_str).unwrap_or("");
            let key = format!("{}:{}:{}", device, destination, gateway);
            let id = record_key(&key);
            self.upsert("route", &id, json!({"key":key,"device_name":device,"destination":destination,"gateway":gateway,"data":route,"observation_id":observation_id,"observed_at":observed_at})).await?;
            self.edge("device_has_route", &device_key, &id, observation_id)
                .await?;
        }
        for acl in array(value, "acls") {
            let name = acl
                .get("name")
                .and_then(Value::as_str)
                .or_else(|| acl.as_str())
                .unwrap_or("unknown");
            let key = format!("{}:{}", device, name);
            let id = record_key(&key);
            self.upsert("acl", &id, json!({"key":key,"device_name":device,"name":name,"data":acl,"observation_id":observation_id,"observed_at":observed_at})).await?;
            self.edge("device_has_acl", &device_key, &id, observation_id)
                .await?;
        }
        for server in array(value, "ntp_servers") {
            let address = server
                .get("address")
                .and_then(Value::as_str)
                .or_else(|| server.as_str())
                .unwrap_or("");
            if address.is_empty() {
                continue;
            }
            let id = record_key(address);
            self.upsert("ntp_server", &id, json!({"key":address,"address":address,"observation_id":observation_id,"observed_at":observed_at})).await?;
            self.edge("device_syncs_with", &device_key, &id, observation_id)
                .await?;
        }
        Ok(())
    }

    pub async fn query_network(&self, query: GraphQuery) -> Result<GraphQueryResult, String> {
        let device = query.device_name.clone();
        let citations = if let Some(device_name) = &device {
            self.select("SELECT * FROM observation WHERE device_name = $value ORDER BY collected_at DESC LIMIT 10;", device_name).await?
        } else {
            Vec::new()
        };
        let latest = citations
            .first()
            .and_then(|v| v.get("collected_at"))
            .and_then(Value::as_str)
            .and_then(|v| DateTime::parse_from_rfc3339(v).ok())
            .map(|v| v.with_timezone(&Utc));
        let fresh = latest
            .map(|time| Utc::now() - time <= Duration::minutes(GRAPH_TTL_MINUTES))
            .unwrap_or(false);
        let mut facts = Vec::new();
        if let Some(device_name) = &device {
            facts.extend(
                self.select("SELECT * FROM device WHERE name = $value;", device_name)
                    .await?,
            );
        }
        if let Some(ip) = &query.ip_address {
            facts.extend(
                self.select("SELECT * FROM ip_address WHERE address = $value;", ip)
                    .await?,
            );
        }
        if let Some(vlan) = query.vlan {
            facts.extend(
                self.select("SELECT * FROM vlan WHERE key = $value;", &vlan.to_string())
                    .await?,
            );
        }
        if let Some(acl) = &query.acl {
            facts.extend(
                self.select("SELECT * FROM acl WHERE name = $value;", acl)
                    .await?,
            );
        }
        if facts.is_empty() && !query.query.trim().is_empty() {
            facts.extend(self.select("SELECT * FROM device WHERE string::lowercase(name) CONTAINS string::lowercase($value);", &query.query).await?);
        }
        let relationships = if let Some(device_name) = &device {
            self.select(
                "SELECT * FROM graph_edge WHERE from = $value;",
                &record_key(device_name),
            )
            .await?
        } else {
            Vec::new()
        };
        Ok(GraphQueryResult {
            fresh,
            requires_refresh: !fresh && device.is_some(),
            facts,
            relationships,
            citations,
        })
    }

    /// Read-through cache lookup used by the fetch tools. Only a committed,
    /// unexpired observation of the requested kind can satisfy this call.
    pub async fn fresh_raw(
        &self,
        device_name: &str,
        kind: GraphDataKind,
    ) -> Result<Option<(String, String)>, String> {
        let mut response = self.db.query(
            "SELECT raw, collected_at FROM observation WHERE device_name = $device AND kind = $kind ORDER BY collected_at DESC LIMIT 1;"
        ).bind(("device", device_name.to_owned())).bind(("kind", kind.as_str())).await
            .map_err(|e| format!("Failed to read graph observation: {e}"))?;
        let records: Vec<Value> = response
            .take(0)
            .map_err(|e| format!("Failed to decode graph observation: {e}"))?;
        let Some(record) = records.first() else {
            return Ok(None);
        };
        let Some(raw) = record.get("raw").and_then(Value::as_str) else {
            return Ok(None);
        };
        let Some(collected_at) = record.get("collected_at").and_then(Value::as_str) else {
            return Ok(None);
        };
        let observed_at = DateTime::parse_from_rfc3339(collected_at)
            .map_err(|e| format!("Invalid graph observation timestamp: {e}"))?
            .with_timezone(&Utc);
        if Utc::now() - observed_at > Duration::minutes(GRAPH_TTL_MINUTES) {
            return Ok(None);
        }
        Ok(Some((raw.to_owned(), collected_at.to_owned())))
    }

    async fn select(&self, sql: &str, value: &str) -> Result<Vec<Value>, String> {
        let mut response = self
            .db
            .query(sql)
            .bind(("value", value.to_owned()))
            .await
            .map_err(|e| format!("Graph query failed: {e}"))?;
        response
            .take(0)
            .map_err(|e| format!("Graph query decoding failed: {e}"))
    }
}

pub fn normalize_yaml(kind: GraphDataKind, yaml: &str) -> Option<Value> {
    let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).ok()?;
    let mut value = serde_json::to_value(parsed).ok()?;
    let obj = value.as_object_mut()?;
    match kind {
        GraphDataKind::Routing => {
            if let Some(routes) = obj.remove("routes") {
                obj.insert("routes".to_string(), routes);
            }
        }
        GraphDataKind::Arp => {
            if let Some(entries) = obj.remove("arp_table") {
                let ips = entries.as_array().map(|items| items.iter().filter_map(|entry| entry.get("ip_address").map(|address| json!({"address":address,"interface":entry.get("interface")}))).collect()).unwrap_or_default();
                obj.insert("ip_addresses".to_string(), Value::Array(ips));
            }
        }
        GraphDataKind::Config => {}
    }
    Some(value)
}

/// Conservative deterministic fallback used when a local model is unavailable.
/// The raw configuration remains the authority; extracted values are limited to
/// unambiguous, common command forms.
pub fn normalize_config(raw: &str) -> Value {
    let mut interfaces = Vec::new();
    let mut ips = Vec::new();
    let mut vlans = Vec::new();
    let mut routes = Vec::new();
    let mut acls = Vec::new();
    let mut ntp_servers = Vec::new();
    let mut current_interface: Option<String> = None;
    for line in raw.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix("interface ") {
            current_interface = Some(name.trim().to_string());
            interfaces.push(json!({"name":name.trim()}));
        } else if let Some(rest) = line.strip_prefix("ip address ") {
            let address = rest.split_whitespace().next().unwrap_or("");
            if !address.is_empty() {
                ips.push(json!({"address":address,"interface":current_interface}));
            }
        } else if let Some(vlan) = line
            .strip_prefix("switchport access vlan ")
            .and_then(|v| v.trim().parse::<u32>().ok())
        {
            vlans.push(json!({"id":vlan,"interface":current_interface}));
        } else if let Some(server) = line.strip_prefix("ntp server ").map(str::trim) {
            if !server.is_empty() {
                ntp_servers
                    .push(json!({"address":server.split_whitespace().next().unwrap_or(server)}));
            }
        } else if let Some(name) = line
            .strip_prefix("access-list ")
            .and_then(|v| v.split_whitespace().next())
        {
            acls.push(json!({"name":name,"line":line}));
        } else if let Some(rest) = line.strip_prefix("ip route ") {
            let mut values = rest.split_whitespace();
            if let Some(destination) = values.next() {
                routes
                    .push(json!({"destination":destination,"gateway":values.nth(1).unwrap_or("")}));
            }
        }
    }
    json!({"interfaces":interfaces,"ip_addresses":ips,"vlans":vlans,"routes":routes,"acls":acls,"ntp_servers":ntp_servers,"normalization":"deterministic_fallback"})
}

/// Prefer the bundled local model for multi-vendor configuration extraction.
/// Model failures retain the raw snapshot and fall back to conservative parsing.
pub async fn normalize_config_with_llm(
    raw: &str,
    app: &tauri::AppHandle,
    state: &crate::llm::llm::LlamaState,
) -> Value {
    const SCHEMA: &str = r#"{"type":"object","properties":{"interfaces":{"type":"array"},"ip_addresses":{"type":"array"},"vlans":{"type":"array"},"routes":{"type":"array"},"acls":{"type":"array"},"ntp_servers":{"type":"array"}},"required":["interfaces","ip_addresses","vlans","routes","acls","ntp_servers"]}"#;
    let prompt = format!(
        "Convert this network device configuration into JSON. Extract only explicit facts; never infer values. Use arrays named interfaces, ip_addresses, vlans, routes, acls, and ntp_servers.\n\nCONFIGURATION:\n{}",
        raw
    );
    match crate::llm::llm::ask_llm_internal_with_schema(
        &prompt,
        "You are a network configuration normalizer. Return valid JSON matching the supplied schema.",
        Some(SCHEMA),
        app,
        state,
    )
    .await
    {
        Ok(output) => serde_json::from_str::<Value>(&output)
            .ok()
            .filter(valid_normalized_config)
            .unwrap_or_else(|| normalize_config(raw)),
        Err(_) => normalize_config(raw),
    }
}

fn valid_normalized_config(value: &Value) -> bool {
    value.as_object().is_some_and(|object| {
        [
            "interfaces",
            "ip_addresses",
            "vlans",
            "routes",
            "acls",
            "ntp_servers",
        ]
        .iter()
        .all(|field| object.get(*field).is_some_and(Value::is_array))
    })
}

/// Chat/MCP entry point.  It deliberately returns structured JSON rather than
/// SurrealQL so callers cannot bypass freshness and provenance rules.
#[tauri::command]
#[allow(non_snake_case)]
pub async fn query_network_graph(
    query: String,
    device_name: Option<String>,
    deviceName: Option<String>,
    ip_address: Option<String>,
    ipAddress: Option<String>,
    vlan: Option<u32>,
    acl: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, SurrealDbState>,
) -> Result<String, String> {
    let request = GraphQuery {
        query,
        device_name: device_name.or(deviceName),
        ip_address: ip_address.or(ipAddress),
        vlan,
        acl,
    };
    let mut result = state.query_network(request.clone()).await?;
    if result.requires_refresh {
        let device = request
            .device_name
            .clone()
            .expect("requires_refresh requires a device");
        // All three collectors are read-only.  They write their raw observation
        // before returning, so a failed collector cannot make old data appear
        // current and a response is only produced after the refresh attempt.
        crate::mcp::fetch::fetch_config::fetch_config(
            app.clone(),
            Some(device.clone()),
            None,
            None,
            None,
            None,
            None,
        )
        .await?;
        let llama_state = app.state::<crate::llm::llm::LlamaState>();
        crate::mcp::fetch::fetch_routing::fetch_routing(
            app.clone(),
            llama_state,
            Some(device.clone()),
            None,
            None,
            None,
            None,
            None,
        )
        .await?;
        let llama_state = app.state::<crate::llm::llm::LlamaState>();
        crate::mcp::fetch::fetch_arp::fetch_arp(
            app.clone(),
            llama_state,
            Some(device),
            None,
            None,
            None,
            None,
            None,
        )
        .await?;
        result = state.query_network(request).await?;
        if result.requires_refresh {
            return Err("Graph refresh did not produce a committed observation; no stale data was returned.".to_string());
        }
    }
    serde_json::to_string_pretty(&result)
        .map_err(|e| format!("Failed to serialize graph result: {e}"))
}

fn array<'a>(value: &'a Value, name: &str) -> &'a [Value] {
    value
        .get(name)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}
fn record_key(input: &str) -> String {
    input
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn fnv1a(input: &str) -> u64 {
    input
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_normalizer_extracts_common_facts() {
        let value = normalize_config("interface Gi0/1\n ip address 10.0.0.1 255.255.255.0\n switchport access vlan 10\nntp server 10.0.0.10\naccess-list EDGE permit ip any any");
        assert_eq!(array(&value, "interfaces").len(), 1);
        assert_eq!(array(&value, "ip_addresses").len(), 1);
        assert_eq!(array(&value, "vlans").len(), 1);
        assert_eq!(array(&value, "ntp_servers").len(), 1);
    }
    #[test]
    fn yaml_normalizer_maps_arp_to_ip_facts() {
        let value = normalize_yaml(
            GraphDataKind::Arp,
            "arp_table:\n  - ip_address: 192.0.2.1\n    interface: eth0",
        )
        .unwrap();
        assert_eq!(array(&value, "ip_addresses")[0]["address"], "192.0.2.1");
    }
}
