use lancedb::connection::Connection;
use lancedb::connect;
use lancedb::query::{ExecutableQuery, QueryBase};
use std::sync::{Arc, Mutex};
use fastembed::{TextEmbedding, EmbeddingModel, InitOptions};
use futures::StreamExt;
use arrow_array::{RecordBatch, StringArray, LargeStringArray, Array};
use tauri::Manager;
use serde::{Serialize, Deserialize};

pub struct RagState {
    pub db: Mutex<Option<Connection>>,
    pub model: Mutex<Option<Arc<TextEmbedding>>>,
}

impl RagState {
    pub fn new() -> Self {
        Self {
            db: Mutex::new(None),
            model: Mutex::new(None),
        }
    }

    pub fn get_model(&self) -> Result<Arc<TextEmbedding>, String> {
        let mut model_lock = self.model.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
        if let Some(model) = &*model_lock {
            return Ok(model.clone());
        }

        let mut options = InitOptions::default();
        options.model_name = EmbeddingModel::MultilingualE5Large;
        options.show_download_progress = true;
        
        let model = TextEmbedding::try_new(options)
            .map_err(|e| format!("Failed to initialize embedding model: {}", e))?;

        let arc_model = Arc::new(model);
        *model_lock = Some(arc_model.clone());
        Ok(arc_model)
    }

    pub async fn get_db(&self, app: &tauri::AppHandle) -> Result<Connection, String> {
        {
            let db_lock = self.db.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
            if let Some(conn) = &*db_lock {
                return Ok(conn.clone());
            }
        }

        let db_path = if let Ok(settings) = crate::settings::load_settings(app.clone()) {
            settings.db_path.filter(|s| !s.trim().is_empty()).unwrap_or_else(|| {
                let app_data_dir = app.path().app_data_dir().expect("Failed to get app data dir");
                app_data_dir.join("lancedb").to_string_lossy().to_string()
            })
        } else {
            let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            app_data_dir.join("lancedb").to_string_lossy().to_string()
        };
        
        let lancedb_dir = std::path::PathBuf::from(&db_path);
        
        if !lancedb_dir.exists() {
            std::fs::create_dir_all(&lancedb_dir).map_err(|e| format!("Failed to create DB directory: {}", e))?;
        }
        
        let path = lancedb_dir.to_string_lossy().to_string();
        
        let conn = connect(&path).execute().await.map_err(|e| format!("DB auto-connect error: {}", e))?;
        
        let mut db_lock = self.db.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
        *db_lock = Some(conn.clone());
        Ok(conn)
    }
}

#[tauri::command]
pub async fn connect_db(path: String, state: tauri::State<'_, RagState>) -> Result<String, String> {
    let conn = connect(&path).execute().await.map_err(|e| format!("DB connect error: {}", e))?;
    
    let mut db_lock = state.db.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    *db_lock = Some(conn);
    
    let _ = state.get_model()?;
    
    Ok("Connected to LanceDB successfully".to_string())
}

#[tauri::command]
pub async fn ingest_document(_path: String) -> Result<String, String> {
    Ok("Document ingested successfully (stub)".to_string())
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RagResult {
    pub success: bool,
    pub output: String,
}

impl From<RagResult> for crate::network::CommandResult {
    fn from(res: RagResult) -> Self {
        Self {
            success: res.success,
            output: res.output,
            saved_path: None,
            is_cached: None,
            cache_time: None,
        }
    }
}


use crate::mcp::brands;
use regex::Regex;

#[tauri::command]
pub async fn query_nw_db(
    query: String, 
    filter: Option<String>, 
    state: tauri::State<'_, RagState>,
    app: tauri::AppHandle
) -> Result<RagResult, String> {
    if let Some(info) = crate::mcp::devices::get_registered_device_info(&query, &app) {
        return Ok(RagResult {
            success: true,
            output: info,
        });
    }

    let mut brand_filter: Option<String> = None;
    let mut processed_query = query.clone();

    // Regex to match [Context: BrandName]
    // Matches something like [Context: Cisco] or [Context: Cisco OS=1.0]
    let context_re = Regex::new(r"\[Context:\s*([^\]\s]+)[^\]]*\]").map_err(|e| e.to_string())?;
    
    if let Some(caps) = context_re.captures(&query) {
        let brand_candidate = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        if let Some(matched_brand) = brands::get_brand(brand_candidate) {
            brand_filter = Some(format!("brand = '{}'", matched_brand));
            // Remove the context tag from the query for embedding
            processed_query = context_re.replace_all(&query, "").to_string().trim().to_string();
        }
    }

    if brand_filter.is_none() {
        // Fallback: check if any known brand name is mentioned in the query string
        for &brand in brands::BRANDS {
            // Case-insensitive word-boundary search
            let brand_re = Regex::new(&format!(r"(?i)\b{}\b", brand)).map_err(|e| e.to_string())?;
            if brand_re.is_match(&query) {
                brand_filter = Some(format!("brand = '{}'", brand));
                break;
            }
        }
    }

    // If query is now empty (e.g. LLM sent ONLY the context tag), 
    // we use a generic query or handle it. 
    // For now, if it's empty, we use the original query's brand name as the query.
    if processed_query.is_empty() && brand_filter.is_some() {
        if let Some(caps) = context_re.captures(&query) {
            processed_query = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
        }
    }

    let db = state.get_db(&app).await?;
    let model = state.get_model()?;

    // E5 models require "query: " prefix for searches
    let instructional_query = format!("query: {}", processed_query);
    let embeddings = model.embed(vec![instructional_query], None)
        .map_err(|e| format!("Embedding error: {}", e))?;
    let query_vector = embeddings.first().ok_or("Failed to generate embedding")?.clone();

    let table = db.open_table("documents").execute().await
        .map_err(|e| format!("Failed to open table: {}", e))?;

    let mut vector_query = table.query()
        .nearest_to(query_vector)
        .map_err(|e| format!("Vector search error: {}", e))?
        .limit(3);
    
    let final_filter = brand_filter.or(filter);
    if let Some(filter_str) = final_filter {
        vector_query = vector_query.only_if(filter_str);
    }

    let mut stream = vector_query.execute().await
        .map_err(|e| format!("Search execution error: {}", e))?;

    let mut context = String::new();
    let mut count = 1;

    while let Some(batch_result) = stream.next().await {
        let batch: RecordBatch = batch_result.map_err(|e| format!("Error reading search results: {}", e))?;
        
        let text_col = batch.column_by_name("text")
            .ok_or("Column 'text' not found in results")?;
            
        let path_col = batch.column_by_name("path")
            .ok_or("Column 'path' not found in results")?;

        let (text_large, text_small) = (
            text_col.as_any().downcast_ref::<LargeStringArray>(),
            text_col.as_any().downcast_ref::<StringArray>(),
        );

        let (path_large, path_small) = (
            path_col.as_any().downcast_ref::<LargeStringArray>(),
            path_col.as_any().downcast_ref::<StringArray>(),
        );

        for i in 0..batch.num_rows() {
            let text = if let Some(arr) = text_large {
                arr.value(i)
            } else if let Some(arr) = text_small {
                arr.value(i)
            } else {
                return Err(format!("Failed to downcast text column. Actual type: {:?}", text_col.data_type()));
            };

            let path = if let Some(arr) = path_large {
                arr.value(i)
            } else if let Some(arr) = path_small {
                arr.value(i)
            } else {
                return Err(format!("Failed to downcast path column. Actual type: {:?}", path_col.data_type()));
            };

            context.push_str(&format!("\n--- 検索結果 {} (ソース: {}) ---\n{}\n", count, path, text));
            count += 1;
        }
    }

    if context.is_empty() {
        Ok(RagResult { success: true, output: "LanceDBに該当する情報が見つかりませんでした。".to_string() })
    } else {
        Ok(RagResult { success: true, output: context })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rag_result_serialization() {
        let result = RagResult {
            success: true,
            output: "RAG search results...".to_string(),
        };
        let serialized = serde_json::to_string(&result).unwrap();
        assert_eq!(serialized, r#"{"success":true,"output":"RAG search results..."}"#);
    }

    #[test]
    fn test_rag_state_instantiation() {
        let state = RagState::new();
        assert!(state.db.lock().unwrap().is_none());
        assert!(state.model.lock().unwrap().is_none());
    }
}
