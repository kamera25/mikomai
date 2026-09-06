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
use validator::Validate;
use crate::graph_identity::{content_hash as fnv1a, record_key};

pub const GRAPH_TTL_MINUTES: i64 = 20;

#[derive(Clone)]
pub struct SurrealDbState {
    pub(crate) db: Surreal<Db>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphDataKind {
    Config,
    Routing,
    Arp,
    Interfaces,
    Lldp,
    MacTable,
    Bgp,
    Ospf,
}

impl GraphDataKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Routing => "routing",
            Self::Arp => "arp",
            Self::Interfaces => "interfaces",
            Self::Lldp => "lldp",
            Self::MacTable => "mac_table",
            Self::Bgp => "bgp",
            Self::Ospf => "ospf",
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
    /// Full canonical document returned by the canonicalization pipeline.
    pub canonical: Option<Value>,
    /// Candidate vectors and source-line references for a canonical result.
    pub evidence: Option<Value>,
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
    /// Canonical documents restored directly from normalized observations.
    pub canonical: Vec<Value>,
}

impl SurrealDbState {
    pub async fn initialize(app: &tauri::AppHandle) -> Result<Self, String> {
        // RAG, graph, and history deliberately share one managed local-first database.
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
DEFINE TABLE rag_chunk SCHEMALESS;
DEFINE ANALYZER rag_text TOKENIZERS class, punct FILTERS lowercase;
DEFINE INDEX device_key ON TABLE device FIELDS key UNIQUE;
DEFINE INDEX observation_device_time ON TABLE observation FIELDS device_name, collected_at;
DEFINE INDEX snapshot_device_time ON TABLE config_snapshot FIELDS device_name, collected_at;
DEFINE INDEX edge_key ON TABLE graph_edge FIELDS key UNIQUE;
DEFINE INDEX rag_chunk_path ON TABLE rag_chunk FIELDS path;
DEFINE INDEX rag_chunk_brand ON TABLE rag_chunk FIELDS brand;
DEFINE INDEX rag_chunk_text ON TABLE rag_chunk FIELDS text FULLTEXT ANALYZER rag_text BM25;
DEFINE INDEX rag_chunk_embedding ON TABLE rag_chunk FIELDS embedding HNSW DIMENSION 1024 DIST COSINE;
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
        if matches!(&input.kind, GraphDataKind::Interfaces) {
            if let Some(canonical) = &input.canonical {
                let table = serde_json::from_value::<crate::schema::interface::UniversalInterfaceTable>(canonical.clone())
                    .map_err(|error| format!("Invalid canonical interface document: {error}"))?;
                table
                    .validate()
                    .map_err(|error| format!("Canonical interface document failed validation: {error}"))?;
            }
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
                "canonical": input.canonical,
                "evidence": input.evidence,
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
        let canonical = citations
            .iter()
            .filter_map(|citation| {
                citation
                    .get("canonical")
                    .filter(|value| !value.is_null())
                    .cloned()
            })
            .collect();
        Ok(GraphQueryResult {
            fresh,
            requires_refresh: !fresh && device.is_some(),
            facts,
            relationships,
            citations,
            canonical,
        })
    }

    /// Returns the most recent raw-only observation for read-through
    /// canonicalization. Raw data is never overwritten.
    pub async fn latest_raw_without_normalized(
        &self,
        device_name: &str,
        kind: GraphDataKind,
    ) -> Result<Option<(String, DateTime<Utc>)>, String> {
        let mut response = self.db.query("SELECT raw, normalized, collected_at FROM observation WHERE device_name = $device AND kind = $kind ORDER BY collected_at DESC LIMIT 10;")
            .bind(("device", device_name.to_owned()))
            .bind(("kind", kind.as_str()))
            .await.map_err(|error| format!("Failed to read graph observation: {error}"))?;
        let records: Vec<Value> = response
            .take(0)
            .map_err(|error| format!("Failed to decode graph observation: {error}"))?;
        for record in records {
            if record
                .get("normalized")
                .is_some_and(|value| !value.is_null())
            {
                continue;
            }
            let Some(raw) = record.get("raw").and_then(Value::as_str) else {
                continue;
            };
            let Some(collected_at) = record.get("collected_at").and_then(Value::as_str) else {
                continue;
            };
            let collected_at = DateTime::parse_from_rfc3339(collected_at)
                .map_err(|error| format!("Invalid graph observation timestamp: {error}"))?
                .with_timezone(&Utc);
            return Ok(Some((raw.to_string(), collected_at)));
        }
        Ok(None)
    }

    pub async fn fresh_canonical(
        &self,
        device_name: &str,
        kind: GraphDataKind,
    ) -> Result<Option<Value>, String> {
        let mut response = self.db.query("SELECT canonical, collected_at FROM observation WHERE device_name = $device AND kind = $kind ORDER BY collected_at DESC LIMIT 10;")
            .bind(("device", device_name.to_owned()))
            .bind(("kind", kind.as_str()))
            .await.map_err(|error| format!("Failed to read canonical observation: {error}"))?;
        let records: Vec<Value> = response
            .take(0)
            .map_err(|error| format!("Failed to decode canonical observation: {error}"))?;
        for record in records {
            let Some(canonical) = record.get("canonical").filter(|value| !value.is_null()) else {
                continue;
            };
            let Some(collected_at) = record.get("collected_at").and_then(Value::as_str) else {
                continue;
            };
            let collected_at = DateTime::parse_from_rfc3339(collected_at)
                .map_err(|error| format!("Invalid graph observation timestamp: {error}"))?
                .with_timezone(&Utc);
            if Utc::now() - collected_at <= Duration::minutes(GRAPH_TTL_MINUTES) {
                return Ok(Some(canonical.clone()));
            }
        }
        Ok(None)
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
        GraphDataKind::Config
        | GraphDataKind::Interfaces
        | GraphDataKind::Lldp
        | GraphDataKind::MacTable
        | GraphDataKind::Bgp
        | GraphDataKind::Ospf => {}
    }
    Some(value)
}

/// Chat/MCP entry point.  It deliberately returns structured JSON rather than
/// SurrealQL so callers cannot bypass freshness and provenance rules.
pub async fn canonicalize_arp_on_read(
    app: &tauri::AppHandle,
    state: &SurrealDbState,
    device_name: &str,
) -> Result<bool, String> {
    let Some((raw, collected_at)) = state
        .latest_raw_without_normalized(device_name, GraphDataKind::Arp)
        .await?
    else {
        return Ok(false);
    };
    let (canonical, normalized, evidence, normalizer_version) =
        if crate::mcp::arp::is_localhost_target(device_name) {
            // Local ARP output is already parsed by the platform-specific
            // collector. Keep canonicalization deterministic and do not make
            // localhost depend on a registered device or an LLM.
            let table = if cfg!(target_os = "windows") {
                crate::mcp::arp::windows::parse_windows_arp(&raw)?
            } else {
                crate::mcp::arp::macos::parse_macos_arp(&raw)?
            };
            let yaml = serde_yaml::to_string(&table)
                .map_err(|error| format!("Failed to serialize local ARP table: {error}"))?;
            let canonical = serde_json::to_value(&table)
                .map_err(|error| format!("Failed to serialize local ARP canonical data: {error}"))?;
            (
                canonical,
                normalize_yaml(GraphDataKind::Arp, &yaml),
                None,
                "arp-local-v1".to_string(),
            )
        } else {
            let os_type = crate::mcp::fetch::fetch_base::resolve_device_config(app, device_name)
                .await
                .map(|config| config.device_type)
                .unwrap_or_else(|_| "unknown".to_string());
            let llama_state = app.state::<crate::llm::llm::LlamaState>();
            let canonicalized = crate::mcp::arp::llm::convert_raw_to_yaml(
                app,
                &llama_state,
                &raw,
                device_name,
                &os_type,
            )
            .await?;
            let canonical = serde_yaml::from_str::<serde_yaml::Value>(&canonicalized.yaml)
                .ok()
                .and_then(|value| serde_json::to_value(value).ok())
                .ok_or_else(|| "Failed to decode canonical ARP YAML".to_string())?;
            (
                canonical,
                normalize_yaml(GraphDataKind::Arp, &canonicalized.yaml),
                Some(
                    serde_json::to_value(canonicalized.evidence)
                        .map_err(|error| format!("Failed to serialize ARP evidence: {error}"))?,
                ),
                "arp-constrained-index-v1".to_string(),
            )
        };
    state
        .ingest(GraphIngestInput {
            source_id: "graph.read_through_canonicalization".to_string(),
            collected_at,
            device_name: device_name.to_string(),
            kind: GraphDataKind::Arp,
            raw,
            normalized,
            canonical: Some(canonical),
            evidence,
            normalizer_version,
        })
        .await?;
    Ok(true)
}

/// Route counterpart to ARP's read-through canonicalization. Raw command
/// output remains authoritative; the constrained result and its line evidence
/// are stored as a distinct observation.
pub async fn canonicalize_route_on_read(
    app: &tauri::AppHandle,
    state: &SurrealDbState,
    device_name: &str,
) -> Result<bool, String> {
    let Some((raw, collected_at)) = state
        .latest_raw_without_normalized(device_name, GraphDataKind::Routing)
        .await?
    else {
        return Ok(false);
    };
    let os_type = crate::mcp::fetch::fetch_base::resolve_device_config(app, device_name)
        .await
        .map(|config| config.device_type)
        .unwrap_or_else(|_| "unknown".to_string());
    let llama_state = app.state::<crate::llm::llm::LlamaState>();
    let canonicalized =
        crate::mcp::route::llm::convert_raw_to_yaml(app, &llama_state, &raw, device_name, &os_type)
            .await?;
    state
        .ingest(GraphIngestInput {
            source_id: "graph.read_through_canonicalization".to_string(),
            collected_at,
            device_name: device_name.to_string(),
            kind: GraphDataKind::Routing,
            raw,
            normalized: normalize_yaml(GraphDataKind::Routing, &canonicalized.yaml),
            canonical: serde_yaml::from_str::<serde_yaml::Value>(&canonicalized.yaml)
                .ok()
                .and_then(|value| serde_json::to_value(value).ok()),
            evidence: Some(
                serde_json::to_value(canonicalized.evidence)
                    .map_err(|error| format!("Failed to serialize route evidence: {error}"))?,
            ),
            normalizer_version: "route-constrained-index-v1".to_string(),
        })
        .await?;
    Ok(true)
}

pub async fn canonicalize_interfaces_on_read(
    app: &tauri::AppHandle,
    state: &SurrealDbState,
    device_name: &str,
) -> Result<bool, String> {
    let Some((raw, collected_at)) = state
        .latest_raw_without_normalized(device_name, GraphDataKind::Interfaces)
        .await?
    else {
        return Ok(false);
    };
    let os_type = crate::mcp::fetch::fetch_base::resolve_device_config(app, device_name)
        .await
        .map(|config| config.device_type)
        .unwrap_or_else(|_| "unknown".to_string());
    let llama_state = app.state::<crate::llm::llm::LlamaState>();
    let canonicalized = crate::mcp::interface::llm::convert_raw_to_yaml(
        app,
        &llama_state,
        &raw,
        device_name,
        &os_type,
    )
    .await?;
    let canonical = serde_yaml::from_str::<serde_yaml::Value>(&canonicalized.yaml)
        .ok()
        .and_then(|value| serde_json::to_value(value).ok());
    state
        .ingest(GraphIngestInput {
            source_id: "graph.read_through_canonicalization".to_string(),
            collected_at,
            device_name: device_name.to_string(),
            kind: GraphDataKind::Interfaces,
            raw,
            normalized: normalize_yaml(GraphDataKind::Interfaces, &canonicalized.yaml),
            canonical,
            evidence: Some(
                serde_json::to_value(canonicalized.evidence)
                    .map_err(|e| format!("Failed to serialize interface evidence: {e}"))?,
            ),
            normalizer_version: "interfaces-constrained-index-v1".to_string(),
        })
        .await?;
    Ok(true)
}

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
        result = state.query_network(request.clone()).await?;
        if result.requires_refresh {
            return Err("Graph refresh did not produce a committed observation; no stale data was returned.".to_string());
        }
    }
    // A graph read is the only place canonicalization is scheduled. This is
    // synchronous by design: it cannot compete with the Agent's next planner
    // inference, and callers receive the persisted canonical document.
    if let Some(device) = request.device_name.as_deref() {
        let normalized_query = request.query.to_ascii_lowercase();
        let query_mentions_arp = normalized_query.contains("arp");
        if query_mentions_arp && canonicalize_arp_on_read(&app, &state, device).await? {
            result = state.query_network(request.clone()).await?;
        }
        let query_mentions_route =
            normalized_query.contains("route") || normalized_query.contains("routing");
        if query_mentions_route && canonicalize_route_on_read(&app, &state, device).await? {
            result = state.query_network(request.clone()).await?;
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
#[cfg(test)]
mod tests {
    use super::*;

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
