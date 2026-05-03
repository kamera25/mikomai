use lancedb::connection::Connection;
use lancedb::connect;
use std::sync::Mutex;
use std::process::Command;
use tauri::Manager;
use serde_json;

pub struct RagState {
    pub db: Mutex<Option<Connection>>,
}

impl RagState {
    pub fn new() -> Self {
        Self {
            db: Mutex::new(None),
        }
    }
}

#[tauri::command]
pub async fn connect_db(path: String, state: tauri::State<'_, RagState>) -> Result<String, String> {
    println!("Connecting to LanceDB at: {}", path);
    let conn = connect(&path).execute().await.map_err(|e| format!("DB connect error: {}", e))?;
    
    let mut db_lock = state.db.lock().unwrap();
    *db_lock = Some(conn);
    
    Ok("Connected to LanceDB successfully".to_string())
}

#[tauri::command]
pub async fn ingest_document(path: String) -> Result<String, String> {
    // This is a stub for the document ingestion pipeline.
    // In a full implementation, this would:
    // 1. Parse PDF/Text
    // 2. Chunk text
    // 3. Generate embeddings using ONNX Runtime
    // 4. Insert into LanceDB
    println!("Ingesting document from: {}", path);
    Ok("Document ingested successfully (stub)".to_string())
}

#[tauri::command]
pub async fn query_rag(query: String, app_handle: tauri::AppHandle) -> Result<String, String> {
    println!("Querying RAG: {}", query);

    let _resource_path = app_handle.path().resource_dir().map_err(|e| e.to_string())?;
    // In dev mode, resource_dir might be the project root. 
    // We need to find the scripts/search_docs.py and venv/bin/python.
    // For simplicity, let's assume we are in the project root in dev mode.
    
    let python_path = "venv/bin/python3";
    let script_path = "scripts/search_docs.py";

    let output = Command::new(python_path)
        .arg(script_path)
        .arg(&query)
        .output()
        .map_err(|e| format!("Failed to execute search script: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Search script error: {}", err));
    }

    let result_json = String::from_utf8_lossy(&output.stdout);
    let results: serde_json::Value = serde_json::from_str(&result_json).map_err(|e| format!("Failed to parse search results: {}", e))?;

    if let Some(err) = results.get("error") {
        return Err(format!("Search error: {}", err));
    }

    let mut context = String::new();
    if let Some(arr) = results.as_array() {
        for (i, res) in arr.iter().enumerate() {
            let text = res.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let path = res.get("path").and_then(|v| v.as_str()).unwrap_or("");
            context.push_str(&format!("\n--- Result {} (Source: {}) ---\n{}\n", i + 1, path, text));
        }
    }

    if context.is_empty() {
        Ok("No relevant information found in LanceDB.".to_string())
    } else {
        Ok(context)
    }
}
