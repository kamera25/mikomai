//! Portable, local-first network graph adapter.
//!
//! This preserves the desktop graph's SurrealDB schema and query semantics
//! while taking an explicit database path instead of a Tauri `AppHandle`.
//! Existing databases can therefore be opened in place by a native host, and
//! observations collected by Swift or another adapter can be ingested here.

use chrono::{DateTime, Duration, Utc};
use crate::router_schema::{self, ROUTER_RESOURCES, ROUTER_SCHEMA_SQL};
use mikomai_core::graph_identity::record_key;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeSet, HashSet},
    net::Ipv4Addr,
    path::Path,
};
use surrealdb::{
    engine::local::{Db, RocksDb},
    types::SurrealValue,
    Surreal,
};

pub const GRAPH_TTL_MINUTES: i64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq,Serialize,Deserialize)]
pub enum EndpointLookup {
    IpByMac,
    MacByIp,
    InterfaceByMac,
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
    MacEntry,
    IpsecConnection,
    IkeSa,
    OspfNeighbor,
    Isis,
    Bfd,
    Ndp,
    Vrrp,
    Lacp,
    Tunnel,
    RoutingPolicy,
    PrefixSet,
    PolicyForwarding,
    AclEntry,
    AclBinding,
    Nat,
    DhcpRelay,
    Qos,
    QosInterface,
    Pim,
    Igmp,
    Mpls,
    DnsServer,
    SyslogServer,
    AaaServer,
    Snmp,
    TelemetrySubscription,
    PlatformComponent,
    System,
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
            Self::MacEntry => "mac_entry",
            Self::IpsecConnection => "ipsec_connection",
            Self::IkeSa => "ike_sa",
            Self::OspfNeighbor => "ospf_neighbor",
            Self::Isis => "isis",
            Self::Bfd => "bfd",
            Self::Ndp => "ndp",
            Self::Vrrp => "vrrp",
            Self::Lacp => "lacp",
            Self::Tunnel => "tunnel",
            Self::RoutingPolicy => "routing_policy",
            Self::PrefixSet => "prefix_set",
            Self::PolicyForwarding => "policy_forwarding",
            Self::AclEntry => "acl_entry",
            Self::AclBinding => "acl_binding",
            Self::Nat => "nat",
            Self::DhcpRelay => "dhcp_relay",
            Self::Qos => "qos",
            Self::QosInterface => "qos_interface",
            Self::Pim => "pim",
            Self::Igmp => "igmp",
            Self::Mpls => "mpls",
            Self::DnsServer => "dns_server",
            Self::SyslogServer => "syslog_server",
            Self::AaaServer => "aaa_server",
            Self::Snmp => "snmp",
            Self::TelemetrySubscription => "telemetry_subscription",
            Self::PlatformComponent => "platform_component",
            Self::System => "system",
        }
    }
}

/// An observation payload accepted from any platform's collectors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphIngestInput {
    pub source_id: String,
    pub collected_at: DateTime<Utc>,
    pub device_name: String,
    pub kind: GraphDataKind,
    pub raw: String,
    pub normalized: Option<Value>,
    pub canonical: Option<Value>,
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
    pub canonical: Vec<Value>,
    pub candidate_devices: Vec<Value>,
}

/// One embedded knowledge chunk stored in the graph's existing `rag_chunk`
/// table.  The embedding is supplied by the platform adapter so the graph
/// remains the single owner of persistence while the model can be selected by
/// the host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagChunkRecord {
    pub path: String,
    pub text: String,
    pub brand: String,
    pub title: String,
    pub summary: String,
    pub os_version: String,
    pub category: String,
    pub command_type: String,
    pub target_model: String,
    pub chunk_index: usize,
    pub embedding: Vec<f32>,
}

/// Candidate returned by vector or full-text search before evidence reranking.
#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct RagChunkCandidate {
    pub path: String,
    pub text: String,
    pub brand: String,
    pub chunk_index: usize,
    pub distance: f32,
}

/// Stored chunk projection used to preview and expand a selected source.
#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct RagDocumentChunk {
    pub path: String,
    pub text: String,
    pub chunk_index: usize,
    pub title: String,
    pub summary: String,
}

fn stable_rag_id(value: &str) -> String {
    let hash = value.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    format!("{hash:016x}")
}

#[derive(Clone)]
pub struct PortableGraph {
    db: Surreal<Db>,
    remote:Option<std::path::PathBuf>,
    operations_lock:std::sync::Arc<tokio::sync::Mutex<()>>,
    documents_lock:std::sync::Arc<tokio::sync::Mutex<()>>,
}

impl PortableGraph {
    /// Open the same `mikomai/network_graph` database used by the retired
    /// desktop runtime. Pass the existing `app_data_dir/surrealdb` path to
    /// reuse its observations; a new directory creates a fresh graph.
    pub async fn initialize_at(path: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create SurrealDB directory: {e}"))?;
        let db = match Surreal::new::<RocksDb>(path).await {
            Ok(db)=>db,
            Err(error)=>{
                #[cfg(unix)]
                if error.to_string().to_ascii_lowercase().contains("lock") {
                    let socket=crate::store_broker::socket_path(path)?;
                    for _ in 0..40 {
                        if crate::store_broker::call::<Value>(&socket,"ping",json!({})).await.is_ok(){return Ok(Self{db:Surreal::init(),remote:Some(socket),operations_lock:Default::default(),documents_lock:Default::default()});}
                        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                    }
                }
                return Err(format!("Failed to open canonical SurrealDB: {error}"));
            }
        };
        db.use_ns("mikomai")
            .use_db("network_graph")
            .await
            .map_err(|e| format!("Failed to select SurrealDB namespace: {e}"))?;
        let state = Self { db,remote:None,operations_lock:Default::default(),documents_lock:Default::default() };
        state.define_schema().await?;
        // Only the process that owns the RocksDB lock performs crash recovery.
        if let Some(mut plans)=state.load_app_document("operations").await? {
            let mut changed=false;
            for plan in plans.as_object_mut().ok_or("invalid operation store")?.values_mut(){if plan["status"]=="executing" {plan["status"]=json!("unknown");changed=true;}}
            if changed {state.commit_operation_state(&plans).await?;}
        }
        #[cfg(unix)]
        crate::store_broker::serve(path,state.clone())?;
        Ok(state)
    }

    async fn define_schema(&self) -> Result<(), String> {
        self.db.query(r#"
DEFINE TABLE IF NOT EXISTS device SCHEMALESS; DEFINE TABLE IF NOT EXISTS interface SCHEMALESS; DEFINE TABLE IF NOT EXISTS ip_address SCHEMALESS;
DEFINE TABLE IF NOT EXISTS subnet SCHEMALESS; DEFINE TABLE IF NOT EXISTS vlan SCHEMALESS; DEFINE TABLE IF NOT EXISTS route SCHEMALESS;
DEFINE TABLE IF NOT EXISTS bgp SCHEMALESS; DEFINE TABLE IF NOT EXISTS vrf SCHEMALESS; DEFINE TABLE IF NOT EXISTS acl SCHEMALESS;
DEFINE TABLE IF NOT EXISTS ntp_server SCHEMALESS; DEFINE TABLE IF NOT EXISTS ntp_status SCHEMALESS; DEFINE TABLE IF NOT EXISTS graph_edge SCHEMALESS;
DEFINE TABLE IF NOT EXISTS observation SCHEMALESS; DEFINE TABLE IF NOT EXISTS config_snapshot SCHEMALESS; DEFINE TABLE IF NOT EXISTS config_change SCHEMALESS;
DEFINE TABLE IF NOT EXISTS conflict SCHEMALESS; DEFINE TABLE IF NOT EXISTS rag_chunk SCHEMALESS;
DEFINE TABLE IF NOT EXISTS tasks SCHEMALESS; DEFINE TABLE IF NOT EXISTS operations SCHEMALESS; DEFINE TABLE IF NOT EXISTS approvals SCHEMALESS; DEFINE TABLE IF NOT EXISTS watches SCHEMALESS; DEFINE TABLE IF NOT EXISTS audit SCHEMALESS; DEFINE TABLE IF NOT EXISTS claims SCHEMALESS;
DEFINE TABLE IF NOT EXISTS settings SCHEMALESS; DEFINE TABLE IF NOT EXISTS sessions SCHEMALESS; DEFINE TABLE IF NOT EXISTS connections SCHEMALESS;
DEFINE ANALYZER IF NOT EXISTS rag_text TOKENIZERS class, punct FILTERS lowercase;
DEFINE INDEX IF NOT EXISTS device_key ON TABLE device FIELDS key UNIQUE;
DEFINE INDEX IF NOT EXISTS observation_device_time ON TABLE observation FIELDS device_name, collected_at;
DEFINE INDEX IF NOT EXISTS edge_key ON TABLE graph_edge FIELDS key UNIQUE;
DEFINE INDEX IF NOT EXISTS rag_chunk_path ON TABLE rag_chunk FIELDS path;
DEFINE INDEX IF NOT EXISTS rag_chunk_brand ON TABLE rag_chunk FIELDS brand;
DEFINE INDEX IF NOT EXISTS rag_chunk_text ON TABLE rag_chunk FIELDS text FULLTEXT ANALYZER rag_text BM25;
DEFINE INDEX IF NOT EXISTS rag_chunk_embedding ON TABLE rag_chunk FIELDS embedding HNSW DIMENSION 1024 DIST COSINE;
"#).await.map_err(|e| format!("Failed to define graph schema: {e}"))?
            .check().map_err(|e| format!("Failed to define graph schema: {e}"))?;
        self.db.query(ROUTER_SCHEMA_SQL).await
            .map_err(|e| format!("Failed to define router schema: {e}"))?
            .check().map_err(|e| format!("Failed to define router schema: {e}"))?;
        Ok(())
    }

    /// Canonical application documents share the graph/RAG database connection.
    pub async fn load_app_document(&self, table: &str) -> Result<Option<Value>, String> {
        #[cfg(unix)]
        if let Some(socket)=&self.remote {return crate::store_broker::call(socket,"load_app_document",json!({"table":table})).await;}

        validate_app_table(table)?;
        let mut response = self.db.query("SELECT schema_version, payload FROM type::record($table, 'current');")
            .bind(("table", table.to_owned())).await.map_err(|e| e.to_string())?
            .check().map_err(|e| e.to_string())?;
        let rows: Vec<AppDocument> = response.take(0).map_err(|e| e.to_string())?;
        rows.into_iter().next().map(|record| {
            if record.schema_version != 1 { return Err(format!("unsupported {table} schema version {}", record.schema_version)); }
            serde_json::from_str(&record.payload).map_err(|e| e.to_string())
        }).transpose()
    }

    /// One UPSERT is atomic. Failure leaves the previous document intact.
    pub async fn save_app_document(&self, table: &str, value: &Value) -> Result<(), String> {
        #[cfg(unix)]
        if let Some(socket)=&self.remote {return crate::store_broker::call(socket,"save_app_document",json!({"table":table,"value":value})).await;}

        let _guard=self.documents_lock.lock().await;
        validate_app_table(table)?;
        // Do not replace data written by a newer application schema.
        self.load_app_document(table).await?;
        let payload = serde_json::to_string(value).map_err(|e| e.to_string())?;
        self.db.query("UPSERT type::record($table, 'current') CONTENT {schema_version: 1, payload: $payload};")
            .bind(("table", table.to_owned())).bind(("payload", payload)).await.map_err(|e| e.to_string())?
            .check().map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Merge disjoint process updates; reject conflicting edits rather than losing them.
    pub async fn merge_app_document(&self,table:&str,previous:&Value,changed:&Value)->Result<(),String>{
        #[cfg(unix)]
        if let Some(socket)=&self.remote{return crate::store_broker::call(socket,"merge_app_document",json!({"table":table,"previous":previous,"changed":changed})).await;}
        let _guard=self.documents_lock.lock().await;
        let current=self.load_app_document(table).await?.unwrap_or(json!({}));
        let merged=merge_document(&current,previous,changed)?;
        let payload=serde_json::to_string(&merged).map_err(|e|e.to_string())?;
        self.db.query("UPSERT type::record($table,'current') CONTENT {schema_version:1,payload:$payload};")
            .bind(("table",table.to_owned())).bind(("payload",payload)).await.map_err(|e|e.to_string())?.check().map_err(|e|e.to_string())?;Ok(())
    }

    /// A plan and its hash-bound approval move together in one transaction.
    pub async fn save_operation_state(&self, plans:&Value)->Result<(),String>{
        #[cfg(unix)]
        if let Some(socket)=&self.remote {return crate::store_broker::call(socket,"save_operation_state",json!({"plans":plans})).await;}

        let _guard=self.operations_lock.lock().await;
        let mut merged=self.load_app_document("operations").await?.unwrap_or(json!({}));
        for (id,plan) in plans.as_object().ok_or("invalid operation state")? {
            if let Some(previous)=merged.get(id) {
                for key in ["id","toolId","target","args","rationale","planHash","operationClass"] {
                    if previous[key]!=plan[key]{return Err("immutable operation plan changed".into());}
                }
                let before=previous["status"].as_str().unwrap_or("");let after=plan["status"].as_str().unwrap_or("");
                if before==after||matches!((before,after),("pending","approved"|"rejected")|("approved","executing")|("executing","executed"|"failed"|"unknown")){merged[id]=plan.clone();}
            } else {merged[id]=plan.clone();}
        }
        self.commit_operation_state(&merged).await
    }
    pub async fn claim_operation(&self,id:&str,hash:&str)->Result<Value,String>{
        #[cfg(unix)]
        if let Some(socket)=&self.remote{return crate::store_broker::call(socket,"claim_operation",json!({"id":id,"hash":hash})).await;}
        let _guard=self.operations_lock.lock().await;
        let mut plans=self.load_app_document("operations").await?.ok_or("operation plan not found")?;
        let plan=plans.get_mut(id).ok_or("operation plan not found")?;
        if plan["status"]!="approved"||plan["planHash"]!=hash{return Err("operation requires an unclaimed matching approval".into());}
        plan["status"]=json!("executing");let claimed=plan.clone();
        self.commit_operation_state(&plans).await?;Ok(claimed)
    }
    async fn commit_operation_state(&self,plans:&Value)->Result<(),String>{
        self.load_app_document("approvals").await?;
        let approvals:serde_json::Map<String,Value>=plans.as_object().ok_or("invalid operation state")?.iter()
            .filter(|(_,plan)|matches!(plan["status"].as_str(),Some("approved"|"executing"|"executed"|"failed"|"unknown")))
            .map(|(id,plan)|(id.clone(),serde_json::json!({"plan_hash":plan["planHash"],"status":plan["status"]}))).collect();
        self.db.query("BEGIN TRANSACTION; UPSERT operations:current CONTENT {schema_version:1,payload:$operations}; UPSERT approvals:current CONTENT {schema_version:1,payload:$approvals}; COMMIT TRANSACTION;")
            .bind(("operations",serde_json::to_string(plans).map_err(|e|e.to_string())?))
            .bind(("approvals",serde_json::to_string(&approvals).map_err(|e|e.to_string())?)).await.map_err(|e|e.to_string())?.check().map_err(|e|e.to_string())?;
        Ok(())
    }

    /// Replace all chunks for a document in the same database used for graph
    /// observations. Chunk record IDs match the previous desktop runtime's
    /// stable FNV-1a path/index IDs.
    pub async fn replace_rag_document(
        &self,
        path: &str,
        chunks: &[RagChunkRecord],
    ) -> Result<(), String> {
        #[cfg(unix)]
        if let Some(socket)=&self.remote {return crate::store_broker::call(socket,"replace_rag_document",json!({"path":path,"chunks":chunks})).await;}

        for chunk in chunks {
            if chunk.path != path {
                return Err(format!("RAG chunk path mismatch while storing {path}"));
            }
            if chunk.embedding.len() != 1024 {
                return Err(format!(
                    "RAG embedding for {path} must contain 1024 values (got {})",
                    chunk.embedding.len()
                ));
            }
        }
        self.db
            .query("DELETE rag_chunk WHERE path = $path;")
            .bind(("path", path.to_owned()))
            .await
            .map_err(|e| format!("Failed to replace existing chunks for {path}: {e}"))?;
        for chunk in chunks {
            let mut record=serde_json::to_value(chunk).map_err(|e|e.to_string())?;record["schema_version"]=serde_json::json!(1);
            let id = stable_rag_id(&format!("{}:{}", chunk.path, chunk.chunk_index));
            self.db
                .query("UPSERT type::record('rag_chunk', $id) CONTENT $record;")
                .bind(("id", id))
                .bind((
                    "record",
                    record,
                ))
                .await
                .map_err(|e| format!("Failed to ingest {path}: {e}"))?;
        }
        Ok(())
    }

    /// Retrieve up to the previous runtime's 30 nearest vector candidates.
    pub async fn search_rag_vectors(
        &self,
        embedding: &[f32],
        brand: Option<&str>,
    ) -> Result<Vec<RagChunkCandidate>, String> {
        #[cfg(unix)]
        if let Some(socket)=&self.remote {return crate::store_broker::call(socket,"search_rag_vectors",json!({"embedding":embedding,"brand":brand})).await;}

        if embedding.len() != 1024 {
            return Err(format!(
                "RAG query embedding must contain 1024 values (got {})",
                embedding.len()
            ));
        }
        let sql = if brand.is_some() {
            "SELECT path, text, brand, chunk_index, vector::distance::knn() AS distance FROM rag_chunk WHERE brand = $brand AND embedding <|30,100|> $embedding LIMIT 30;"
        } else {
            "SELECT path, text, brand, chunk_index, vector::distance::knn() AS distance FROM rag_chunk WHERE embedding <|30,100|> $embedding LIMIT 30;"
        };
        let mut response = self
            .db
            .query(sql)
            .bind(("embedding", embedding.to_vec()))
            .bind(("brand", brand.unwrap_or_default().to_owned()))
            .await
            .map_err(|e| format!("SurrealDB vector search failed: {e}"))?;
        response
            .take(0)
            .map_err(|e| format!("Failed to decode SurrealDB vector search results: {e}"))
    }

    /// Retrieve full-text candidates. These supplement vector results so exact
    /// command names remain discoverable, then the RAG reranker evaluates them.
    pub async fn search_rag_lexical(
        &self,
        query: &str,
        brand: Option<&str>,
    ) -> Result<Vec<RagChunkCandidate>, String> {
        #[cfg(unix)]
        if let Some(socket)=&self.remote {return crate::store_broker::call(socket,"search_rag_lexical",json!({"query":query,"brand":brand})).await;}

        let sql = if brand.is_some() {
            "SELECT path, text, brand, chunk_index, 2.0 AS distance FROM rag_chunk WHERE brand = $brand AND text @1@ $query LIMIT 30;"
        } else {
            "SELECT path, text, brand, chunk_index, 2.0 AS distance FROM rag_chunk WHERE text @1@ $query LIMIT 30;"
        };
        let mut response = self
            .db
            .query(sql)
            .bind(("brand", brand.unwrap_or_default().to_owned()))
            .bind(("query", query.to_owned()))
            .await
            .map_err(|e| format!("SurrealDB full-text search failed: {e}"))?;
        response
            .take(0)
            .map_err(|e| format!("Failed to decode SurrealDB full-text search results: {e}"))
    }

    pub async fn rag_document_chunks(&self, path: &str) -> Result<Vec<RagDocumentChunk>, String> {
        #[cfg(unix)]
        if let Some(socket)=&self.remote {return crate::store_broker::call(socket,"rag_document_chunks",json!({"path":path})).await;}

        let mut response = self
            .db
            .query("SELECT path, text, chunk_index, title, summary FROM rag_chunk WHERE path = $path ORDER BY chunk_index ASC;")
            .bind(("path", path.to_owned()))
            .await
            .map_err(|e| format!("Failed to read RAG document {path}: {e}"))?;
        response
            .take(0)
            .map_err(|e| format!("Failed to decode RAG document {path}: {e}"))
    }

    async fn upsert(&self, table: &str, key: &str, mut record: Value) -> Result<(), String> {
        record["schema_version"]=serde_json::json!(1);
        let sql = format!("UPSERT type::record('{table}', $id) CONTENT $record;");
        self.db
            .query(sql)
            .bind(("id", key.to_owned()))
            .bind(("record", record))
            .await
            .map_err(|e| format!("Failed to write {table}: {e}"))?
            .check().map_err(|e| format!("Failed to write {table}: {e}"))?;
        Ok(())
    }

    async fn edge(
        &self,
        kind: &str,
        from: &str,
        to: &str,
        observation_id: &str,
    ) -> Result<(), String> {
        let key = format!("{kind}:{from}:{to}");
        self.upsert(
            "graph_edge",
            &record_key(&key),
            json!({
                "key":key,"kind":kind,"from":from,"to":to,"observation_id":observation_id,
                "updated_at":Utc::now().to_rfc3339()
            }),
        )
        .await
    }

    async fn select(&self, sql: &str, value: &str) -> Result<Vec<Value>, String> {
        let mut result = self
            .db
            .query(sql)
            .bind(("value", value.to_owned()))
            .await
            .map_err(|e| format!("Graph query failed: {e}"))?;
        result
            .take(0)
            .map_err(|e| format!("Graph query decoding failed: {e}"))
    }

    /// Persist a collected observation while preserving raw data, provenance,
    /// canonical output, and a device record compatible with the old graph.
    pub async fn ingest(&self, input: GraphIngestInput) -> Result<(), String> {
        #[cfg(unix)]
        if let Some(socket)=&self.remote {return crate::store_broker::call(socket,"ingest",json!({"input":input})).await;}

        if input.device_name.trim().is_empty() || input.source_id.trim().is_empty() {
            return Err("Graph ingestion requires a device name and source ID".into());
        }
        if let Some(value) = &input.normalized { router_schema::validate_normalized(value)?; }
        // A retry/canonicalization update for the same source and collection
        // time replaces its raw observation instead of creating an ambiguous tie.
        let observation_id = stable_rag_id(&format!("{}:{}:{}:{}", input.device_name, input.kind.as_str(), input.source_id, input.collected_at.to_rfc3339()));
        self.upsert("observation", &observation_id, json!({
            "id":observation_id, "source_id":input.source_id, "device_name":input.device_name,
            "kind":input.kind.as_str(), "collected_at":input.collected_at.to_rfc3339(), "raw":input.raw,
            "normalized":input.normalized, "canonical":input.canonical, "evidence":input.evidence,
            "normalizer_version":input.normalizer_version
        })).await?;
        if let Some(normalized) = input.normalized.as_ref() {
            self.store_normalized(
                &input.device_name,
                normalized,
                &observation_id,
                input.collected_at,
            )
            .await?;
        }
        let key = record_key(&input.device_name);
        self.upsert(
            "device",
            &key,
            json!({"key":key,"name":input.device_name,
            "observation_id":observation_id,"observed_at":input.collected_at.to_rfc3339()}),
        )
        .await?;
        Ok(())
    }

    /// Resolve an observation ID without TTL refresh, collection or inference.
    pub async fn state_snapshot(
        &self,
        id: &str,
        device: &str,
        resource: &str,
        scope: &str,
    ) -> Result<Option<mikomai_core::network::state::StateSnapshot>, String> {
        #[cfg(unix)]
        if let Some(socket) = &self.remote {
            return crate::store_broker::call(
                socket,
                "state_snapshot",
                json!({"id":id,"device":device,"resource":resource,"scope":scope}),
            )
            .await;
        }
        crate::state::resource_shape(resource)?;
        if resource != "interfaces" && scope != "all" {
            return Err("Only interfaces support acquisition scopes".into());
        }
        let source = if resource == "interfaces" {
            format!("get_state.interfaces:{scope}")
        } else {
            String::new()
        };
        let response=self.db.query("SELECT record::id(id) AS snapshot_id, device_name, kind, source_id, collected_at, normalizer_version, canonical FROM observation WHERE device_name = $device AND kind = $resource AND ($source = '' OR source_id = $source) AND ($snapshot = 'latest' OR record::id(id) = $snapshot) ORDER BY collected_at DESC, snapshot_id ASC LIMIT 1;")
            .bind(("device",device.to_owned())).bind(("resource",resource.to_owned())).bind(("source",source)).bind(("snapshot",id.to_owned())).await.map_err(|e|e.to_string())?;
        let mut response = response.check().map_err(|e| e.to_string())?;
        let rows: Vec<Value> = response.take(0).map_err(|e| e.to_string())?;
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        let text = |key: &str| {
            row[key]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("Invalid stored snapshot metadata: {key}"))
        };
        let canonical = row.get("canonical").filter(|v| !v.is_null()).cloned();
        let collected_at = text("collected_at")?;
        let timestamp = DateTime::parse_from_rfc3339(&collected_at)
            .map_err(|e| e.to_string())?
            .with_timezone(&Utc);
        // Existing collectors publish canonical data only after completeness validation.
        let known_normalizer = match resource {
            "interfaces" => text("normalizer_version")? == "interface-constrained-index-v1",
            "arp" => text("normalizer_version")? == "arp-constrained-index-v2",
            _ => text("normalizer_version")? == crate::router_canonicalization::VERSION,
        };
        let complete = known_normalizer
            && canonical.as_ref().is_some_and(|value| match resource {
                "interfaces" => {
                    mikomai_core::network::interface::validate_canonical_table(value, device)
                        .is_ok()
                }
                "arp" => {
                    mikomai_core::network::arp::validate_canonical_table(value, device).is_ok()
                }
                _ => crate::router_canonicalization::validate_canonical(
                    value, resource, device, timestamp,
                )
                .is_ok(),
            });
        Ok(Some(mikomai_core::network::state::StateSnapshot {
            snapshot_id: text("snapshot_id")?,
            device: device.into(),
            resource: resource.into(),
            scope: scope.into(),
            collected_at,
            source_id: text("source_id")?,
            normalizer_version: text("normalizer_version")?,
            complete,
            canonical,
        }))
    }

    /// Only the latest observation of this device/resource is authoritative.
    /// Never resurrect an older canonical table after a newer failed refresh.
    pub async fn fresh_arp_observation(&self, device: &str) -> Result<Option<mikomai_core::network::arp_state::ArpObservation>, String> {
        #[cfg(unix)]
        if let Some(socket)=&self.remote {return crate::store_broker::call(socket,"fresh_arp_observation",json!({"device":device})).await;}

        let mut response = self.db.query("SELECT raw, canonical, collected_at FROM observation WHERE device_name = $device AND kind = 'arp' ORDER BY collected_at DESC LIMIT 10;")
            .bind(("device", device.to_owned())).await.map_err(|e| e.to_string())?;
        let records: Vec<Value> = response.take(0).map_err(|e| e.to_string())?;
        let Some(row) = records.first() else { return Ok(None); };
        let Some(timestamp) = row["collected_at"].as_str() else { return Ok(None); };
        let row = records.iter().find(|candidate| candidate["collected_at"].as_str() == Some(timestamp) && !candidate["canonical"].is_null()).unwrap_or(row);
        let collected_at = DateTime::parse_from_rfc3339(timestamp).map_err(|e| e.to_string())?.with_timezone(&Utc);
        let age = Utc::now() - collected_at;
        if age < Duration::zero() || age > Duration::minutes(GRAPH_TTL_MINUTES) { return Ok(None); }
        Ok(Some(mikomai_core::network::arp_state::ArpObservation {
            raw: row["raw"].as_str().unwrap_or_default().to_owned(),
            canonical: row.get("canonical").filter(|value| !value.is_null()).cloned(),
            collected_at,
        }))
    }

    /// Latest observation for this explicit scope. Raw-only failed
    /// canonicalization must never resurrect an older successful state.
    pub async fn latest_interface_observation(&self, device: &str, scope: &str) -> Result<Option<mikomai_core::network::interface_state::InterfaceObservation>,String> {
        #[cfg(unix)]
        if let Some(socket)=&self.remote {return crate::store_broker::call(socket,"latest_interface_observation",json!({"device":device,"scope":scope})).await;}

        let response = self.db.query("SELECT raw, canonical, collected_at FROM observation WHERE device_name = $device AND kind = 'interfaces' AND source_id = $source ORDER BY collected_at DESC LIMIT 1;")
            .bind(("device",device.to_owned())).bind(("source",format!("get_state.interfaces:{scope}"))).await.map_err(|e|e.to_string())?;
        let mut response = response.check().map_err(|e|e.to_string())?;
        let records:Vec<Value> = response.take(0).map_err(|e|e.to_string())?;
        let Some(row)=records.first() else {return Ok(None);};
        let collected_at=DateTime::parse_from_rfc3339(row["collected_at"].as_str().ok_or("観測時刻がありません")?).map_err(|e|e.to_string())?.with_timezone(&Utc);
        Ok(Some(mikomai_core::network::interface_state::InterfaceObservation {raw:row["raw"].as_str().unwrap_or_default().into(),
            canonical:row.get("canonical").filter(|v|!v.is_null()).cloned(),collected_at}))
    }

    /// Latest resource observation, including raw-only or failed refreshes.
    pub async fn latest_router_observation(&self, device: &str, table: &str) -> Result<Option<crate::router_state::RouterObservation>, String> {
        #[cfg(unix)]
        if let Some(socket)=&self.remote {return crate::store_broker::call(socket,"latest_router_observation",json!({"device":device,"table":table})).await;}

        router_schema::resource_schema(table)?;
        let response = self.db.query("SELECT raw, canonical, collected_at, source_id FROM observation WHERE device_name = $device AND kind = $kind ORDER BY collected_at DESC LIMIT 1;")
            .bind(("device", device.to_owned())).bind(("kind", table.to_owned())).await.map_err(|e| e.to_string())?;
        let mut response = response.check().map_err(|e| e.to_string())?;
        let records: Vec<Value> = response.take(0).map_err(|e| e.to_string())?;
        let Some(row) = records.first() else { return Ok(None); };
        Ok(Some(crate::router_state::RouterObservation {
            raw: row["raw"].as_str().unwrap_or_default().into(),
            canonical: row.get("canonical").filter(|v| !v.is_null()).cloned(),
            collected_at: DateTime::parse_from_rfc3339(row["collected_at"].as_str().ok_or("Missing observation timestamp")?).map_err(|e|e.to_string())?.with_timezone(&Utc),
            source_id: row["source_id"].as_str().ok_or("Missing observation source")?.into(),
        }))
    }

    async fn store_normalized(
        &self,
        device: &str,
        value: &Value,
        observation_id: &str,
        observed_at: DateTime<Utc>,
    ) -> Result<(), String> {
        let device_key = record_key(device);
        for interface in json_array(value, "interfaces") {
            let name = interface
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let key = format!("{device}:{name}");
            let id = record_key(&key);
            self.upsert("interface", &id, json!({"key":key,"device_name":device,"name":name,"data":interface,"observation_id":observation_id,"observed_at":observed_at.to_rfc3339()})).await?;
            self.edge("has_interface", &device_key, &id, observation_id)
                .await?;
        }
        for ip in json_array(value, "ip_addresses") {
            let address = ip
                .get("address")
                .and_then(Value::as_str)
                .or_else(|| ip.as_str())
                .unwrap_or("");
            if address.is_empty() {
                continue;
            }
            let id = record_key(address);
            self.upsert("ip_address", &id, json!({"key":address,"address":address,"data":ip,"observation_id":observation_id,"observed_at":observed_at.to_rfc3339()})).await?;
            self.edge("device_has_ip", &device_key, &id, observation_id)
                .await?;
            if let Some(subnet) = ip.get("subnet").and_then(Value::as_str) {
                let subnet_id = record_key(subnet);
                self.upsert("subnet", &subnet_id, json!({"key":subnet,"cidr":subnet,"observation_id":observation_id,"observed_at":observed_at.to_rfc3339()})).await?;
                self.edge("ip_in_subnet", &id, &subnet_id, observation_id)
                    .await?;
            }
        }
        for vlan in json_array(value, "vlans") {
            let number = vlan
                .get("id")
                .and_then(Value::as_u64)
                .or_else(|| vlan.as_u64())
                .map(|v| v.to_string());
            let Some(number) = number else {
                continue;
            };
            let id = record_key(&number);
            self.upsert("vlan", &id, json!({"key":number,"data":vlan,"observation_id":observation_id,"observed_at":observed_at.to_rfc3339()})).await?;
            self.edge("device_has_vlan", &device_key, &id, observation_id)
                .await?;
        }
        for route in json_array(value, "routes") {
            let destination = route
                .get("destination")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let gateway = route.get("gateway").and_then(Value::as_str).unwrap_or("");
            let key = format!("{device}:{destination}:{gateway}");
            let id = record_key(&key);
            self.upsert("route", &id, json!({"key":key,"device_name":device,"destination":destination,"gateway":gateway,"data":route,"observation_id":observation_id,"observed_at":observed_at.to_rfc3339()})).await?;
            self.edge("device_has_route", &device_key, &id, observation_id)
                .await?;
        }
        for resource in ROUTER_RESOURCES.iter() {
            for row in json_array(value, &resource.table) {
                let mut identity = vec![json!(resource.table), json!(device)];
                identity.extend(resource.identity.iter().map(|name| row[name].clone()));
                let key = Value::Array(identity).to_string();
                let id = record_key(&key);
                let mut record = row.clone();
                record["schema_version"] = json!(1);
                record["key"] = json!(key);
                record["device_name"] = json!(device);
                record["observation_id"] = json!(observation_id);
                record["observed_at"] = json!(observed_at.to_rfc3339());
                let kind = format!("device_has_{}", resource.table);
                let edge_key = format!("{kind}:{device_key}:{id}");
                let edge = json!({"schema_version":1,"key":edge_key,"kind":kind,"from":device_key,"to":id,
                    "observation_id":observation_id,"updated_at":observed_at.to_rfc3339()});
                // Delayed observations cannot replace newer feature state.
                self.db.query("BEGIN TRANSACTION;
                    LET $previous = (SELECT VALUE observed_at FROM type::record($table, $id))[0];
                    IF $previous = NONE OR $previous <= $record.observed_at {
                        UPSERT type::record($table, $id) CONTENT $record;
                        UPSERT type::record('graph_edge', $edge_id) CONTENT $edge;
                    };
                    COMMIT TRANSACTION;")
                    .bind(("table",resource.table.clone())).bind(("id",id))
                    .bind(("record",record)).bind(("edge_id",record_key(&edge_key))).bind(("edge",edge))
                    .await.map_err(|e| format!("Failed to store {}: {e}", resource.table))?
                    .check().map_err(|e| format!("Failed to store {}: {e}", resource.table))?;
            }
        }
        Ok(())
    }

    /// Native feature facts with observation provenance. Callers must check
    /// observed_at; persisted facts do not imply current live device state.
    pub async fn router_facts(&self, table: &str, device: &str) -> Result<Vec<Value>, String> {
        #[cfg(unix)]
        if let Some(socket)=&self.remote {return crate::store_broker::call(socket,"router_facts",json!({"table":table,"device":device})).await;}

        let resource = router_schema::resource_schema(table)?;
        self.select(&format!("SELECT * FROM {} WHERE device_name = $value ORDER BY key;", resource.table), device).await
    }

    pub async fn query_network(&self, query: GraphQuery) -> Result<GraphQueryResult, String> {
        #[cfg(unix)]
        if let Some(socket)=&self.remote {return crate::store_broker::call(socket,"query_network",json!({"query":query})).await;}

        let citations = if let Some(device) = &query.device_name {
            let mut res = self.db.query("SELECT * FROM observation WHERE device_name = $device ORDER BY collected_at DESC LIMIT 10;")
                .bind(("device", device.clone())).await.map_err(|e| e.to_string())?;
            res.take(0).map_err(|e| e.to_string())?
        } else {
            vec![]
        };
        let latest = citations
            .first()
            .and_then(|v: &Value| v.get("collected_at"))
            .and_then(Value::as_str)
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc));
        let mut fresh = latest
            .map(|t| Utc::now() - t <= Duration::minutes(GRAPH_TTL_MINUTES))
            .unwrap_or(false);
        let mut facts = Vec::new();
        if let Some(device) = &query.device_name {
            facts.extend(
                self.select("SELECT * FROM device WHERE name = $value;", device)
                    .await?,
            );
            if router_schema::resource_schema(query.query.trim()).is_ok() {
                let resources = self.router_facts(query.query.trim(), device).await?;
                fresh = !resources.is_empty() && resources.iter().all(|row| {
                    row["observed_at"].as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok())
                        .is_some_and(|t| {
                            let age = Utc::now() - t.with_timezone(&Utc);
                            age >= Duration::zero() && age <= Duration::minutes(GRAPH_TTL_MINUTES)
                        })
                });
                facts.extend(resources);
            }
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
        let relationships = if let Some(device) = &query.device_name {
            self.select(
                "SELECT * FROM graph_edge WHERE from = $value;",
                &record_key(device),
            )
            .await?
        } else {
            vec![]
        };
        let canonical = citations
            .iter()
            .filter_map(|item| item.get("canonical").filter(|v| !v.is_null()).cloned())
            .collect();
        let candidate_devices = if let Some(ip) = &query.ip_address {
            self.devices_for_ip(ip).await?
        } else {
            vec![]
        };
        Ok(GraphQueryResult {
            fresh,
            requires_refresh: !fresh && query.device_name.is_some(),
            facts,
            relationships,
            citations,
            canonical,
            candidate_devices,
        })
    }

    async fn devices_for_ip(&self, ip: &str) -> Result<Vec<Value>, String> {
        let target: Ipv4Addr = match ip.parse() {
            Ok(ip) => ip,
            Err(_) => return Ok(vec![]),
        };
        let mut response = self.db.query("SELECT device_name, canonical, collected_at FROM observation WHERE kind = 'interfaces' ORDER BY collected_at DESC LIMIT 200;")
            .await.map_err(|e| e.to_string())?;
        let records: Vec<Value> = response.take(0).map_err(|e| e.to_string())?;
        let (mut seen, mut candidates) = (HashSet::new(), vec![]);
        for record in records {
            let Some(device) = record.get("device_name").and_then(Value::as_str) else {
                continue;
            };
            let Some(interfaces) = record
                .get("canonical")
                .and_then(|v| v.get("interfaces"))
                .and_then(Value::as_array)
            else {
                continue;
            };
            if !seen.insert(device.to_owned()) {
                continue;
            }
            for intf in interfaces {
                let prefix = intf.get("prefix_len").and_then(Value::as_u64);
                for address in intf
                    .get("ipv4_addresses")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let Some(text) = address.as_str() else {
                        continue;
                    };
                    let (base, inline) = text
                        .split_once('/')
                        .map_or((text, None), |(a, b)| (a, Some(b)));
                    let Some(bits) = inline
                        .and_then(|v| v.parse::<u32>().ok())
                        .or(prefix.map(|p| p as u32))
                        .filter(|p| *p <= 32)
                    else {
                        continue;
                    };
                    let Ok(base_ip) = base.parse::<Ipv4Addr>() else {
                        continue;
                    };
                    let mask = if bits == 0 {
                        0
                    } else {
                        u32::MAX << (32 - bits)
                    };
                    if u32::from(target) & mask == u32::from(base_ip) & mask {
                        candidates.push(json!({"device_name":device,"interface":intf.get("name"),"subnet":format!("{base}/{bits}"),"collected_at":record.get("collected_at")}));
                    }
                }
            }
        }
        Ok(candidates)
    }

    /// Resolve IP/MAC/port matches only from recent committed observations.
    pub async fn find_endpoint(
        &self,
        lookup: EndpointLookup,
        value: &str,
        device: Option<&str>,
    ) -> Result<Value, String> {
        #[cfg(unix)]
        if let Some(socket)=&self.remote {return crate::store_broker::call(socket,"find_endpoint",json!({"lookup":lookup,"value":value,"device":device})).await;}

        let needle = match lookup {
            EndpointLookup::MacByIp => value
                .parse::<std::net::IpAddr>()
                .map_err(|_| "A valid IP address is required")?
                .to_string(),
            _ => normalize_mac(value).ok_or("A valid MAC address is required")?,
        };
        let kind = if lookup == EndpointLookup::InterfaceByMac {
            "mac_table"
        } else {
            "arp"
        };
        let mut response = self.db.query("SELECT device_name, kind, canonical, raw, collected_at, source_id FROM observation WHERE kind = $kind ORDER BY collected_at DESC LIMIT 200;")
            .bind(("kind",kind)).await.map_err(|e| e.to_string())?;
        let observations: Vec<Value> = response.take(0).map_err(|e| e.to_string())?;
        let (mut seen, mut matches) = (HashSet::new(), vec![]);
        for row in observations {
            let Some(name) = row.get("device_name").and_then(Value::as_str) else {
                continue;
            };
            if device.is_some_and(|d| !d.eq_ignore_ascii_case(name)) {
                continue;
            }
            if lookup != EndpointLookup::InterfaceByMac
                && row
                    .get("canonical")
                    .and_then(|v| v.get("arp_table"))
                    .is_none()
            {
                continue;
            }
            if !seen.insert(name.to_owned()) {
                continue;
            }
            let Some(timestamp) = row.get("collected_at").and_then(Value::as_str) else {
                continue;
            };
            let Ok(timestamp_parsed) = DateTime::parse_from_rfc3339(timestamp) else {
                continue;
            };
            if Utc::now() - timestamp_parsed.with_timezone(&Utc)
                > Duration::minutes(GRAPH_TTL_MINUTES)
            {
                continue;
            }
            if lookup == EndpointLookup::InterfaceByMac {
                if let Some(raw) = row.get("raw").and_then(Value::as_str) {
                    for port in mac_table_ports(raw, &needle) {
                        matches.push(json!({"device_name":name,"mac_address":needle,"interface":port,"collected_at":timestamp,"source_id":row.get("source_id")}));
                    }
                }
            } else if let Some(entries) = row
                .get("canonical")
                .and_then(|v| v.get("arp_table"))
                .and_then(Value::as_array)
            {
                for entry in entries {
                    let ip = entry.get("ip_address").and_then(Value::as_str);
                    let mac = entry
                        .get("mac_address")
                        .and_then(Value::as_str)
                        .and_then(normalize_mac);
                    let matched = match lookup {
                        EndpointLookup::MacByIp => ip == Some(&needle) && mac.is_some(),
                        EndpointLookup::IpByMac => mac.as_deref() == Some(&needle),
                        EndpointLookup::InterfaceByMac => false,
                    };
                    if matched {
                        matches.push(json!({"device_name":name,"ip_address":ip,"mac_address":mac,"interface":entry.get("interface"),"collected_at":timestamp,"source_id":row.get("source_id")}));
                    }
                }
            }
        }
        Ok(
            json!({"query":value,"matches":matches,"found":!matches.is_empty(),"resource":kind,"requires_refresh":matches.is_empty()}),
        )
    }

    pub async fn get_subgraph(&self, request: SubgraphRequest) -> Result<SubgraphResult, String> {
        #[cfg(unix)]
        if let Some(socket)=&self.remote {return crate::store_broker::call(socket,"get_subgraph",json!({"request":request})).await;}

        request.validate()?;
        let mut result = SubgraphResult {
            nodes: vec![],
            edges: vec![],
            missing_roots: vec![],
        };
        let mut visited = BTreeSet::new();
        for root in request.roots.iter().collect::<BTreeSet<_>>() {
            if self
                .select("SELECT * FROM device WHERE name = $value;", root)
                .await?
                .is_empty()
            {
                result.missing_roots.push(root.clone());
            } else {
                visited.insert(record_key(root));
            }
        }
        let kinds: Vec<String> = request
            .relations
            .iter()
            .flat_map(|r| r.kinds())
            .map(|kind| kind.to_owned())
            .collect();
        let mut frontier = visited.clone();
        let mut edge_keys = BTreeSet::new();
        for _ in 0..request.depth {
            if frontier.is_empty() {
                break;
            }
            let mut response = self.db.query("SELECT * FROM graph_edge WHERE kind IN $kinds AND (from IN $frontier OR to IN $frontier) ORDER BY key LIMIT 2001;")
                .bind(("kinds",kinds.clone())).bind(("frontier",frontier.into_iter().collect::<Vec<_>>())).await.map_err(|e|e.to_string())?;
            let edges: Vec<Value> = response.take(0).map_err(|e| e.to_string())?;
            if edges.len() > 2000 {
                return Err("Subgraph exceeds 2000 edges; reduce roots or depth".into());
            }
            frontier = BTreeSet::new();
            for edge in edges {
                for endpoint in ["from", "to"] {
                    if let Some(id) = edge[endpoint].as_str() {
                        if visited.insert(id.to_owned()) {
                            frontier.insert(id.to_owned());
                        }
                    }
                }
                if let Some(key) = edge["key"].as_str() {
                    if edge_keys.insert(key.to_owned()) {
                        result.edges.push(edge);
                    }
                }
            }
            if visited.len() > 1000 || result.edges.len() > 2000 {
                return Err(
                    "Subgraph exceeds 1000 nodes or 2000 edges; reduce roots or depth".into(),
                );
            }
        }
        for table in ["device", "interface", "bgp", "vrf", "route"].into_iter()
            .chain(ROUTER_RESOURCES.iter().map(|resource| resource.table.as_str())) {
            let sql=format!("SELECT *, record::id(id) AS traversal_id FROM {table} WHERE record::id(id) IN $ids ORDER BY key;");
            let mut response = self
                .db
                .query(sql)
                .bind(("ids", visited.iter().cloned().collect::<Vec<_>>()))
                .await
                .map_err(|e| e.to_string())?;
            let nodes: Vec<Value> = response.take(0).map_err(|e| e.to_string())?;
            for mut node in nodes {
                let id = node
                    .as_object_mut()
                    .and_then(|n| n.remove("traversal_id"))
                    .unwrap_or(Value::Null);
                result
                    .nodes
                    .push(json!({"node_id":id,"type":table,"record":node}));
            }
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, Serialize,Deserialize)]
pub struct SubgraphRequest {
    pub roots: Vec<String>,
    pub depth: u8,
    pub relations: Vec<SubgraphRelation>,
}
#[derive(Debug, Clone, Copy, Serialize,Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubgraphRelation {
    Interface,
    Bgp,
    Vrf,
    Route,
    Ospf,
    MacEntry,
    IpsecConnection,
    IkeSa,
    OspfNeighbor,
    Isis,
    Bfd,
    Lldp,
    Ndp,
    Vrrp,
    Lacp,
    Tunnel,
    RoutingPolicy,
    PrefixSet,
    PolicyForwarding,
    AclEntry,
    AclBinding,
    Nat,
    DhcpRelay,
    Qos,
    QosInterface,
    Pim,
    Igmp,
    Mpls,
    DnsServer,
    SyslogServer,
    AaaServer,
    Snmp,
    TelemetrySubscription,
    PlatformComponent,
    System,
}
impl SubgraphRelation {
    fn kinds(self) -> Vec<String> {
        let table = match self {
            Self::Interface => return vec!["has_interface".into(), "interface".into()],
            Self::Bgp => return vec!["device_has_bgp".into(), "bgp".into()],
            Self::Vrf => return vec!["device_has_vrf".into(), "vrf".into()],
            Self::Route => return vec!["device_has_route".into(), "route".into()],
            Self::Ospf => "ospf",
            Self::MacEntry => "mac_entry",
            Self::IpsecConnection => "ipsec_connection",
            Self::IkeSa => "ike_sa",
            Self::OspfNeighbor => "ospf_neighbor",
            Self::Isis => "isis",
            Self::Bfd => "bfd",
            Self::Lldp => "lldp",
            Self::Ndp => "ndp",
            Self::Vrrp => "vrrp",
            Self::Lacp => "lacp",
            Self::Tunnel => "tunnel",
            Self::RoutingPolicy => "routing_policy",
            Self::PrefixSet => "prefix_set",
            Self::PolicyForwarding => "policy_forwarding",
            Self::AclEntry => "acl_entry",
            Self::AclBinding => "acl_binding",
            Self::Nat => "nat",
            Self::DhcpRelay => "dhcp_relay",
            Self::Qos => "qos",
            Self::QosInterface => "qos_interface",
            Self::Pim => "pim",
            Self::Igmp => "igmp",
            Self::Mpls => "mpls",
            Self::DnsServer => "dns_server",
            Self::SyslogServer => "syslog_server",
            Self::AaaServer => "aaa_server",
            Self::Snmp => "snmp",
            Self::TelemetrySubscription => "telemetry_subscription",
            Self::PlatformComponent => "platform_component",
            Self::System => "system",
        };
        vec![format!("device_has_{table}")]
    }
}

#[derive(Debug, Clone, Serialize,Deserialize)]
pub struct SubgraphResult {
    pub nodes: Vec<Value>,
    pub edges: Vec<Value>,
    pub missing_roots: Vec<String>,
}
impl SubgraphRequest {
    fn validate(&self) -> Result<(), String> {
        if self.roots.is_empty()
            || self.roots.len() > 32
            || self.roots.iter().any(|r| r.trim().is_empty())
        {
            return Err("get_subgraph roots must contain 1–32 non-empty device names".into());
        }
        if self.depth > 8 || self.relations.is_empty() {
            return Err("get_subgraph requires depth 0–8 and non-empty relations".into());
        }
        Ok(())
    }
}

/// This tool communicates the host-registration requirement to the caller.
/// Native UI code owns registration and presents the appropriate flow.
pub fn require_host_registered() -> mikomai_core::port::ToolResult {
    mikomai_core::port::ToolResult {
        success: false,
        output:
            "ホスト名の登録が必要です。IPアドレスおよびFQDNを直接指定したリモート接続は行えません。"
                .into(),
    }
}

fn json_array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn normalize_mac(value: &str) -> Option<String> {
    let digits: String = value.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if digits.len() != 12
        || !value
            .chars()
            .all(|c| c.is_ascii_hexdigit() || matches!(c, ':' | '-' | '.'))
    {
        return None;
    }
    let digits = digits.to_ascii_lowercase();
    Some(
        (0..6)
            .map(|i| &digits[i * 2..i * 2 + 2])
            .collect::<Vec<_>>()
            .join(":"),
    )
}
fn mac_table_ports(raw: &str, mac: &str) -> Vec<String> {
    let mut ports = vec![];
    for line in raw.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 2
            || !fields
                .iter()
                .any(|f| normalize_mac(f).as_deref() == Some(mac))
        {
            continue;
        }
        if let Some(port) = fields.last() {
            if normalize_mac(port).is_none() && !ports.iter().any(|p| p == port) {
                ports.push((*port).to_owned());
            }
        }
    }
    ports
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn native_router_tables_enforce_schema_and_preserve_scoped_facts() {
        let path = std::env::temp_dir().join(format!("mikomai-router-schema-{}", uuid::Uuid::new_v4()));
        let graph = PortableGraph::initialize_at(&path).await.unwrap();
        graph.define_schema().await.unwrap();
        let now = Utc::now();
        let input = |device: &str, normalized: Value, time: DateTime<Utc>| GraphIngestInput {
            source_id: "test:native".into(), collected_at: time, device_name: device.into(),
            kind: GraphDataKind::Config, raw: "fixture".into(), normalized: Some(normalized),
            canonical: None, evidence: Some(json!({"transport":"fixture"})), normalizer_version: "test".into(),
        };
        // Exercise every actual table, typed column and index against RocksDB.
        let mut normalized = serde_json::Map::new();
        let planner_schema: Value = serde_json::from_str(&mikomai_core::planner::build_decision_schema(
            &["r1".into()], &["get_subgraph".into()])).unwrap();
        for resource in ROUTER_RESOURCES.iter() {
            let relation: SubgraphRelation = serde_json::from_value(json!(resource.table)).unwrap();
            assert_eq!(relation.kinds(), vec![format!("device_has_{}", resource.table)]);
            assert!(planner_schema["properties"]["parameters"]["properties"]["relations"]["items"]["enum"]
                .as_array().unwrap().contains(&json!(resource.table)));
            let kind: GraphDataKind = serde_json::from_value(json!(resource.table)).unwrap();
            assert_eq!(kind.as_str(), resource.table);
            let mut row = serde_json::Map::new();
            for field in &resource.fields {
                let value = match field.name.as_str() {
                    "version" => json!(2), "direction" => json!("ingress"),
                    "address_family" => json!("ipv6"), "port" => json!(514),
                    "mtu" => json!(1500), "virtual_router_id" => json!(42),
                    "initiator_spi" | "responder_spi" => json!("18446744073709551615"),
                    _ => match field.field_type.as_str() {
                        "string" => json!("sample"), "int" => json!(1), "float" => json!(20.5),
                        "bool" => json!(true), "object" => json!({"nested":{"packets":42}}),
                        "array<object>" => json!([{"nested":{"packets":42}}]),
                        "array<string>" => json!(["sample"]), "array<int>" => json!([16]),
                        _ => unreachable!(),
                    },
                };
                row.insert(field.name.clone(), value);
            }
            normalized.insert(resource.table.clone(), json!([row]));
        }
        graph.ingest(input("fixtures", Value::Object(normalized.clone()), now)).await.unwrap();
        for resource in ROUTER_RESOURCES.iter() {
            let rows = graph.router_facts(&resource.table, "fixtures").await.unwrap();
            assert_eq!(rows.len(), 1, "{}", resource.table);
            for (name, expected) in normalized[&resource.table][0].as_object().unwrap() {
                assert_eq!(&rows[0][name], expected, "{}.{}", resource.table, name);
            }
            assert!(rows[0]["observation_id"].is_string());
        }
        let snapshot = json!({"ospf":[
            {"vrf":"blue","version":2,"process_id":"1","router_id":"192.0.2.1","enabled":true},
            {"vrf":"red","version":2,"process_id":"1","router_id":"192.0.2.2","enabled":false}
        ],"lldp":[{"interface":"Gi0/1","neighbor_id":"peer-1","chassis_id":"aa:bb:cc:dd:ee:ff",
            "port_id":"Gi0/2","system_name":"switch1","capabilities":["BRIDGE"],"ttl":120}]});
        graph.ingest(input("r1", snapshot.clone(), now)).await.unwrap();
        graph.ingest(input("r2", snapshot.clone(), now)).await.unwrap();
        graph.ingest(input("r1", snapshot, now)).await.unwrap();
        assert_eq!(graph.router_facts("ospf", "r1").await.unwrap().len(), 2);
        assert_eq!(graph.router_facts("ospf", "r2").await.unwrap().len(), 2);
        let subgraph = graph.get_subgraph(SubgraphRequest {roots:vec!["r1".into()],depth:1,
            relations:vec![SubgraphRelation::Ospf,SubgraphRelation::Lldp]}).await.unwrap();
        assert_eq!(subgraph.edges.len(), 3);
        assert_eq!(subgraph.nodes.len(), 4);
        assert!(subgraph.nodes.iter().any(|node| node["type"] == "lldp" && node["record"]["system_name"] == "switch1"));
        // Out-of-order observations retain the latest fact and its citation edge.
        graph.ingest(input("r1",json!({"ospf":[{"vrf":"blue","version":2,"process_id":"1","enabled":false}]}),now-Duration::minutes(1))).await.unwrap();
        let rows = graph.router_facts("ospf", "r1").await.unwrap();
        assert_eq!(rows.iter().find(|r| r["vrf"] == "blue").unwrap()["enabled"], true);
        let query = graph.query_network(GraphQuery {query:"ospf".into(),device_name:Some("r1".into()),
            ip_address:None,vlan:None,acl:None}).await.unwrap();
        assert!(query.fresh);
        assert_eq!(query.facts.len(), 3);
        graph.ingest(input("stale",json!({"lldp":[{"interface":"Gi0/1","neighbor_id":"peer-1"}]}),now-Duration::minutes(30))).await.unwrap();
        graph.ingest(input("stale",json!({"interfaces":[{"name":"Gi0/1"}]}),now)).await.unwrap();
        let stale = graph.query_network(GraphQuery {query:"lldp".into(),device_name:Some("stale".into()),
            ip_address:None,vlan:None,acl:None}).await.unwrap();
        assert!(!stale.fresh && stale.requires_refresh);
        for invalid in [
            json!({"ospf":[{"vrf":"blue","version":2}]}),
            json!({"lldp":[{"interface":"Gi0/1","neighbor_id":"peer","ttl":"120"}]}),
            json!({"lldp":[{"interface":"Gi0/1","neighbor_id":"peer","device_name":"forged"}]}),
            json!({"lldp":{}}),
            json!({"ospf":[{"vrf":"blue","version":4,"process_id":"1"}]}),
            json!({"lldp":[{"interface":"Gi0/1","neighbor_id":"peer","ttl":65536}]}),
            json!({"lldp":[{"interface":"Gi0/1","neighbor_id":"peer"},{"interface":"Gi0/1","neighbor_id":"peer"}]}),
        ] {
            assert!(graph.ingest(input("invalid",invalid,now)).await.is_err());
        }
        assert!(graph.select("SELECT * FROM observation WHERE device_name = $value;", "invalid").await.unwrap().is_empty());
        assert!(graph.router_facts("lldp; DELETE device", "r1").await.is_err());
        assert!(graph.router_facts("openconfig", "r1").await.is_err());
        // The database enforces types and protocol ranges independently of ingestion.
        let mut bad = rows[0].clone();
        bad.as_object_mut().unwrap().remove("id");
        bad["version"] = json!(4);
        assert!(graph.upsert("ospf", "bad-version",bad.clone()).await.is_err());
        bad["version"] = json!("2");
        assert!(graph.upsert("ospf", "bad-type",bad).await.is_err());
        let mut response = graph.db.query("INFO FOR DB;").await.unwrap().check().unwrap();
        let info: Option<Value> = response.take(0).unwrap();
        let info = info.unwrap();
        assert!(info["tables"].get("ospf").is_some());
        assert!(info["tables"].get("lldp").is_some());
        assert!(info["tables"].get("openconfig").is_none());
        drop(graph);
        let mut reopened = None;
        for _ in 0..100 {
            match PortableGraph::initialize_at(&path).await {
                Ok(graph) => { reopened = Some(graph); break; }
                Err(error) if error.contains("LOCK") || error.contains("lock") =>
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await,
                Err(error) => panic!("Reopening graph failed: {error}"),
            }
        }
        let graph = reopened.expect("RocksDB should release its lock after drop");
        assert_eq!(graph.router_facts("ospf", "r2").await.unwrap().len(), 2);
        drop(graph);
        let _ = std::fs::remove_dir_all(path);
    }
    #[test]
    fn mac_formats_normalize_and_mac_table_extracts_ports() {
        assert_eq!(
            normalize_mac("AABB.CCDD.EEFF").as_deref(),
            Some("aa:bb:cc:dd:ee:ff")
        );
        assert_eq!(
            mac_table_ports("10 aabb.ccdd.eeff DYNAMIC Gi1/0/2", "aa:bb:cc:dd:ee:ff"),
            vec!["Gi1/0/2"]
        );
        assert!(normalize_mac("not-a-mac").is_none());
    }
    #[tokio::test]
    async fn ingested_observations_drive_graph_queries_and_endpoint_lookups() {
        let path =
            std::env::temp_dir().join(format!("mikomai-portable-graph-{}", uuid::Uuid::new_v4()));
        let graph = PortableGraph::initialize_at(&path).await.unwrap();
        graph
            .ingest(GraphIngestInput {
                source_id: "swift:test".into(),
                collected_at: Utc::now(),
                device_name: "gw01".into(),
                kind: GraphDataKind::Interfaces,
                raw: String::new(),
                normalized: Some(json!({"interfaces":[{"name":"vlan10","ipv4_addresses":["10.0.0.1/24"]}],"routes":[{"destination":"10.1.0.0/16","gateway":"10.0.0.2"}]})),
                canonical: Some(
                    json!({"interfaces":[{"name":"vlan10","ipv4_addresses":["10.0.0.1/24"]}]}),
                ),
                evidence: Some(json!({"source":"test"})),
                normalizer_version: "test".into(),
            })
            .await
            .unwrap();
        let graph_result = graph
            .query_network(GraphQuery {
                query: "10.0.0.10".into(),
                device_name: None,
                ip_address: Some("10.0.0.10".into()),
                vlan: None,
                acl: None,
            })
            .await
            .unwrap();
        assert_eq!(graph_result.candidate_devices[0]["device_name"], "gw01");
        let subgraph = graph
            .get_subgraph(SubgraphRequest {
                roots: vec!["gw01".into()],
                depth: 1,
                relations: vec![SubgraphRelation::Interface, SubgraphRelation::Route],
            })
            .await
            .unwrap();
        assert_eq!(subgraph.edges.len(), 2);
        assert!(subgraph
            .nodes
            .iter()
            .any(|node| node["type"] == "interface"));
        assert!(subgraph.nodes.iter().any(|node| node["type"] == "route"));
        graph.ingest(GraphIngestInput { source_id:"swift:arp".into(),collected_at:Utc::now(),device_name:"gw01".into(),kind:GraphDataKind::Arp,raw:String::new(),normalized:None,
            canonical:Some(json!({"arp_table":[{"ip_address":"10.0.0.10","mac_address":"aa:bb:cc:dd:ee:ff","interface":"vlan10"}]})),evidence:None,normalizer_version:"test".into() }).await.unwrap();
        let by_mac = graph
            .find_endpoint(EndpointLookup::IpByMac, "AA-BB-CC-DD-EE-FF", None)
            .await
            .unwrap();
        assert_eq!(by_mac["matches"][0]["ip_address"], "10.0.0.10");
        let by_ip = graph
            .find_endpoint(EndpointLookup::MacByIp, "10.0.0.10", Some("GW01"))
            .await
            .unwrap();
        assert_eq!(by_ip["matches"][0]["mac_address"], "aa:bb:cc:dd:ee:ff");
        graph
            .ingest(GraphIngestInput {
                source_id: "swift:mac".into(),
                collected_at: Utc::now(),
                device_name: "sw01".into(),
                kind: GraphDataKind::MacTable,
                raw: "10 aabb.ccdd.eeff DYNAMIC Gi1/0/2".into(),
                normalized: None,
                canonical: None,
                evidence: None,
                normalizer_version: "test".into(),
            })
            .await
            .unwrap();
        let port = graph
            .find_endpoint(EndpointLookup::InterfaceByMac, "aa:bb:cc:dd:ee:ff", None)
            .await
            .unwrap();
        assert_eq!(port["matches"][0]["interface"], "Gi1/0/2");
        drop(graph);
        let _ = std::fs::remove_dir_all(path);
    }
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct AppDocument { schema_version: u32, payload: String }

fn validate_app_table(table: &str) -> Result<(), String> {
    if ["settings", "sessions", "connections", "tasks", "operations", "approvals", "watches", "audit", "claims"].contains(&table) { Ok(()) }
    else { Err("unsupported application collection".into()) }
}

#[cfg(test)]
mod app_document_tests {
    use super::*;
    #[tokio::test]
    async fn documents_share_the_graph_store_and_reject_unknown_collections() {
        let path = std::env::temp_dir().join(format!("mikomai-documents-{}", uuid::Uuid::new_v4()));
        let db = PortableGraph::initialize_at(&path).await.unwrap();
        assert!(db.load_app_document("sessions").await.unwrap().is_none());
        let payload = json!({"sessions":[{"id":"test","messages":[{"text":"日本語\nquoted"}]}], "activeSessionID":"test"});
        db.save_app_document("sessions", &payload).await.unwrap();
        assert_eq!(db.clone().load_app_document("sessions").await.unwrap(), Some(payload.clone()));
        assert!(db.save_app_document("sessions;DELETE device", &json!({})).await.is_err());
        assert_eq!(db.load_app_document("sessions").await.unwrap(), Some(payload));
        db.db.query("UPSERT settings:current CONTENT {schema_version: 999, payload: '{}'};").await.unwrap().check().unwrap();
        assert!(db.load_app_document("settings").await.unwrap_err().contains("unsupported"));
        assert!(db.save_app_document("settings", &json!({})).await.unwrap_err().contains("unsupported"));
        assert!(db.load_app_document("settings").await.unwrap_err().contains("999"));
    }
}

fn merge_document(current:&Value,previous:&Value,changed:&Value)->Result<Value,String>{
    if previous==changed{return Ok(current.clone());}
    if current==previous{return Ok(changed.clone());}
    let empty = serde_json::Map::new();
    let previous_object = if previous.is_null() { Some(&empty) } else { previous.as_object() };
    if let (Some(current),Some(previous),Some(changed))=(current.as_object(),previous_object,changed.as_object()) {
        let mut merged=current.clone();let keys=previous.keys().chain(changed.keys()).collect::<BTreeSet<_>>();
        for key in keys {if previous.get(key)==changed.get(key){continue;}
            if let Some(value)=changed.get(key){merged.insert(key.clone(),merge_document(current.get(key).unwrap_or(&Value::Null),previous.get(key).unwrap_or(&Value::Null),value)?);}
            else {if current.get(key)!=previous.get(key){return Err("canonical document changed; reload before editing".into());}merged.remove(key);}
        }return Ok(Value::Object(merged));
    }
    if let (Some(current),Some(previous),Some(changed))=(current.as_array(),previous.as_array(),changed.as_array()) {
        if current.starts_with(previous)&&changed.starts_with(previous){let mut merged=current.clone();merged.extend_from_slice(&changed[previous.len()..]);return Ok(Value::Array(merged));}
    }
    Err("canonical document changed; reload before editing".into())
}

#[cfg(test)]
mod canonical_concurrency_tests {
    use super::*;
    #[tokio::test]
    async fn clients_merge_disjoint_updates_claim_once_and_rollback_transaction_failure(){
        let path=std::env::temp_dir().join(format!("mikomai-canonical-{}",uuid::Uuid::new_v4()));
        let owner=PortableGraph::initialize_at(&path).await.unwrap();
        let client=PortableGraph::initialize_at(&path).await.unwrap();
        owner.merge_app_document("tasks",&json!({}),&json!({"scheduled":{"one":{"seq":1}}})).await.unwrap();
        client.merge_app_document("tasks",&json!({}),&json!({"scheduled":{"two":{"seq":1}}})).await.unwrap();
        let tasks=owner.load_app_document("tasks").await.unwrap().unwrap();assert_eq!(tasks["scheduled"].as_object().unwrap().len(),2);
        let mut plan=mikomai_core::OperationPlan::new("network_config",Some("fixture".into()),json!({"commands":["hostname fixture"]}),"test").unwrap();
        let hash=plan.plan_hash.clone();mikomai_core::OperationGate::approve(&mut plan,&hash).unwrap();let id=plan.id.to_string();
        owner.save_operation_state(&json!({id.clone():plan})).await.unwrap();
        assert!(client.claim_operation(&id,"wrong").await.is_err());
        owner.db.query("DEFINE FIELD OVERWRITE payload ON TABLE approvals TYPE int;").await.unwrap().check().unwrap();
        assert!(client.claim_operation(&id,&hash).await.is_err());
        assert_eq!(owner.load_app_document("operations").await.unwrap().unwrap()[&id]["status"],"approved");
        owner.db.query("REMOVE FIELD payload ON TABLE approvals;").await.unwrap().check().unwrap();
        assert_eq!(client.claim_operation(&id,&hash).await.unwrap()["status"],"executing");
        assert!(owner.claim_operation(&id,&hash).await.is_err());
        assert_eq!(owner.load_app_document("approvals").await.unwrap().unwrap()[&id]["status"],"executing");
    }
}
