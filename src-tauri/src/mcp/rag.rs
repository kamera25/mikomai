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
        let mut model_lock = self.model.lock().unwrap();
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
            let db_lock = self.db.lock().unwrap();
            if let Some(conn) = &*db_lock {
                return Ok(conn.clone());
            }
        }

        let db_path = if let Ok(settings) = crate::settings::load_settings(app.clone()) {
            settings.db_path.unwrap_or_else(|| {
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
        
        let mut db_lock = self.db.lock().unwrap();
        *db_lock = Some(conn.clone());
        Ok(conn)
    }
}

#[tauri::command]
pub async fn connect_db(path: String, state: tauri::State<'_, RagState>) -> Result<String, String> {
    let conn = connect(&path).execute().await.map_err(|e| format!("DB connect error: {}", e))?;
    
    let mut db_lock = state.db.lock().unwrap();
    *db_lock = Some(conn);
    
    let _ = state.get_model()?;
    
    Ok("Connected to LanceDB successfully".to_string())
}

#[tauri::command]
pub async fn ingest_document(path: String) -> Result<String, String> {
    Ok("Document ingested successfully (stub)".to_string())
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RagResult {
    pub success: bool,
    pub output: String,
}

#[tauri::command]
pub async fn query_nw_db(
    query: String, 
    filter: Option<String>, 
    state: tauri::State<'_, RagState>,
    app: tauri::AppHandle
) -> Result<RagResult, String> {

    let db = state.get_db(&app).await?;
    let model = state.get_model()?;

    // E5 models require "query: " prefix for searches
    let instructional_query = format!("query: {}", query);
    let embeddings = model.embed(vec![instructional_query], None)
        .map_err(|e| format!("Embedding error: {}", e))?;
    let query_vector = embeddings.first().ok_or("Failed to generate embedding")?.clone();

    let table = db.open_table("documents").execute().await
        .map_err(|e| format!("Failed to open table: {}", e))?;

    let mut vector_query = table.query()
        .nearest_to(query_vector)
        .map_err(|e| format!("Vector search error: {}", e))?
        .limit(3);
    
    if let Some(filter_str) = filter {
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
            
        let text_values: Vec<String> = if let Some(arr) = text_col.as_any().downcast_ref::<LargeStringArray>() {
            (0..arr.len()).map(|i| arr.value(i).to_string()).collect()
        } else if let Some(arr) = text_col.as_any().downcast_ref::<StringArray>() {
            (0..arr.len()).map(|i| arr.value(i).to_string()).collect()
        } else {
            return Err(format!("Failed to downcast text column. Actual type: {:?}", text_col.data_type()));
        };

        let path_col = batch.column_by_name("path")
            .ok_or("Column 'path' not found in results")?;
            
        let path_values: Vec<String> = if let Some(arr) = path_col.as_any().downcast_ref::<LargeStringArray>() {
            (0..arr.len()).map(|i| arr.value(i).to_string()).collect()
        } else if let Some(arr) = path_col.as_any().downcast_ref::<StringArray>() {
            (0..arr.len()).map(|i| arr.value(i).to_string()).collect()
        } else {
            return Err(format!("Failed to downcast path column. Actual type: {:?}", path_col.data_type()));
        };

        for i in 0..batch.num_rows() {
            let text = &text_values[i];
            let path = &path_values[i];
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
