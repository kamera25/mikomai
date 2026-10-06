//! Same-user Unix broker: one canonical embedded DB, including concurrent GUI/CLI.
use crate::portable_graph::*;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
const LIMIT: usize = 32 * 1024 * 1024;
pub fn socket_path(database: &Path) -> Result<PathBuf, String> {
    let uid = unsafe { libc::geteuid() };
    let root = PathBuf::from(format!("/tmp/mikomai-store-{uid}"));
    match std::fs::create_dir(&root) {
        Ok(()) => std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.to_string()),
    }
    let metadata = std::fs::symlink_metadata(&root).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
        return Err("canonical store socket directory has unsafe ownership/permissions".into());
    }
    let database = database.canonicalize().map_err(|e| e.to_string())?;
    Ok(root.join(format!(
        "{:x}.sock",
        Sha256::digest(database.as_os_str().as_encoded_bytes())
    )))
}
async fn read_frame(stream: &mut tokio::net::UnixStream) -> Result<Value, String> {
    let count = stream
        .read_u32()
        .await
        .map_err(|_| "store connection closed")? as usize;
    if count > LIMIT {
        return Err("store message exceeds limit".into());
    }
    let mut bytes = vec![0; count];
    stream
        .read_exact(&mut bytes)
        .await
        .map_err(|_| "store response incomplete")?;
    serde_json::from_slice(&bytes).map_err(|_| "invalid store response".into())
}
async fn write_frame(stream: &mut tokio::net::UnixStream, value: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() > LIMIT {
        return Err("store message exceeds limit".into());
    }
    stream
        .write_u32(bytes.len() as u32)
        .await
        .map_err(|_| "store write failed")?;
    stream
        .write_all(&bytes)
        .await
        .map_err(|_| "store write failed".into())
}
pub async fn call<T: DeserializeOwned>(
    path: &Path,
    method: &str,
    args: Value,
) -> Result<T, String> {
    tokio::time::timeout(std::time::Duration::from_secs(60), async {
        let mut stream = tokio::net::UnixStream::connect(path)
            .await
            .map_err(|_| "canonical store owner unavailable")?;
        write_frame(
            &mut stream,
            &json!({"version":1,"method":method,"args":args}),
        )
        .await?;
        let value = read_frame(&mut stream).await?;
        if value["version"] != 1 {
            return Err("unsupported store protocol".into());
        }
        if let Some(error) = value["error"].as_str() {
            return Err(error.into());
        }
        serde_json::from_value(value["value"].clone()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "canonical store deadline exceeded".to_string())?
}
pub fn serve(database: &Path, graph: PortableGraph) -> Result<(), String> {
    let path = socket_path(database)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    let listener = tokio::net::UnixListener::bind(&path).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let graph = graph.clone();
            tokio::spawn(async move {
                let result = tokio::time::timeout(std::time::Duration::from_secs(60), async {
                    let request = read_frame(&mut stream).await?;
                    if request["version"] != 1 {
                        return Err("unsupported store protocol".into());
                    }
                    dispatch(
                        &graph,
                        request["method"].as_str().unwrap_or(""),
                        &request["args"],
                    )
                    .await
                })
                .await
                .unwrap_or_else(|_| Err("store request deadline exceeded".into()));
                let response = match result {
                    Ok(value) => json!({"version":1,"value":value}),
                    Err(error) => json!({"version":1,"error":error}),
                };
                let _ = write_frame(&mut stream, &response).await;
            });
        }
    });
    Ok(())
}
fn encode<T: Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}
fn decode<T: DeserializeOwned>(value: &Value) -> Result<T, String> {
    serde_json::from_value(value.clone()).map_err(|e| e.to_string())
}
fn dispatch<'a>(
    graph: &'a PortableGraph,
    method: &'a str,
    args: &'a Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, String>> + Send + 'a>> {
    Box::pin(async move {
        let text = |key: &str| args[key].as_str().unwrap_or("");
        match method {
            "ping" => Ok(Value::Null),
            "load_app_document" => encode(graph.load_app_document(text("table")).await?),
            "save_app_document" => encode(
                graph
                    .save_app_document(text("table"), &args["value"])
                    .await?,
            ),
            "merge_app_document" => encode(
                graph
                    .merge_app_document(text("table"), &args["previous"], &args["changed"])
                    .await?,
            ),
            "save_operation_state" => encode(graph.save_operation_state(&args["plans"]).await?),
            "claim_operation" => encode(graph.claim_operation(text("id"), text("hash")).await?),
            "replace_rag_document" => encode(
                graph
                    .replace_rag_document(
                        text("path"),
                        &decode::<Vec<RagChunkRecord>>(&args["chunks"])?,
                    )
                    .await?,
            ),
            "search_rag_vectors" => encode(
                graph
                    .search_rag_vectors(
                        &decode::<Vec<f32>>(&args["embedding"])?,
                        args["brand"].as_str(),
                    )
                    .await?,
            ),
            "search_rag_lexical" => encode(
                graph
                    .search_rag_lexical(text("query"), args["brand"].as_str())
                    .await?,
            ),
            "rag_document_chunks" => encode(graph.rag_document_chunks(text("path")).await?),
            "ingest" => encode(graph.ingest(decode(&args["input"])?).await?),
            "fresh_arp_observation" => encode(graph.fresh_arp_observation(text("device")).await?),
            "latest_interface_observation" => encode(
                graph
                    .latest_interface_observation(text("device"), text("scope"))
                    .await?,
            ),
            "latest_router_observation" => encode(
                graph
                    .latest_router_observation(text("device"), text("table"))
                    .await?,
            ),
            "router_facts" => encode(graph.router_facts(text("table"), text("device")).await?),
            "query_network" => encode(graph.query_network(decode(&args["query"])?).await?),
            "find_endpoint" => encode(
                graph
                    .find_endpoint(
                        decode(&args["lookup"])?,
                        text("value"),
                        args["device"].as_str(),
                    )
                    .await?,
            ),
            "get_subgraph" => encode(graph.get_subgraph(decode(&args["request"])?).await?),
            _ => Err("unsupported canonical store request".into()),
        }
    })
}
