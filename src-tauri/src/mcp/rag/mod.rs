pub mod full_text;
pub mod traits;
pub mod vector;
pub mod vendor;

pub use full_text::FullTextSearcher;
pub use traits::RagSearcher;
pub use vector::VectorSearcher;

use arrow_array::{Array, Float32Array, LargeStringArray, RecordBatch, StringArray};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use lancedb::connect;
use lancedb::connection::Connection;
use std::sync::{Arc, Mutex};
use tauri::Manager;

pub struct RagState
{
    pub db: Mutex<Option<Connection>>,
    pub model: Mutex<Option<Arc<TextEmbedding>>>,
}

impl RagState
{
    pub fn new() -> Self
    {
        Self {
            db: Mutex::new(None),
            model: Mutex::new(None),
        }
    }

    pub fn get_model(&self) -> Result<Arc<TextEmbedding>, String>
    {
        let mut model_lock = self
            .model
            .lock()
            .map_err(|_| "Mutex lock poisoned".to_string())?;
        if let Some(model) = &*model_lock
        {
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

    pub async fn get_db(&self, app: &tauri::AppHandle) -> Result<Connection, String>
    {
        {
            let db_lock = self
                .db
                .lock()
                .map_err(|_| "Mutex lock poisoned".to_string())?;
            if let Some(conn) = &*db_lock
            {
                return Ok(conn.clone());
            }
        }

        let db_path = if let Ok(settings) = crate::settings::load_settings(app.clone())
        {
            settings
                .db_path
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| {
                    let app_data_dir = app
                        .path()
                        .app_data_dir()
                        .expect("Failed to get app data dir");
                    app_data_dir.join("lancedb").to_string_lossy().to_string()
                })
        }
        else
        {
            let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            app_data_dir.join("lancedb").to_string_lossy().to_string()
        };

        let lancedb_dir = std::path::PathBuf::from(&db_path);

        if !lancedb_dir.exists()
        {
            std::fs::create_dir_all(&lancedb_dir)
                .map_err(|e| format!("Failed to create DB directory: {}", e))?;
        }

        let path = lancedb_dir.to_string_lossy().to_string();

        let conn = connect(&path)
            .execute()
            .await
            .map_err(|e| format!("DB auto-connect error: {}", e))?;

        let mut db_lock = self
            .db
            .lock()
            .map_err(|_| "Mutex lock poisoned".to_string())?;
        *db_lock = Some(conn.clone());
        Ok(conn)
    }
}

impl Default for RagState
{
    fn default() -> Self
    {
        Self::new()
    }
}

#[tauri::command]
pub async fn connect_db(path: String, state: tauri::State<'_, RagState>) -> Result<String, String>
{
    let conn = connect(&path)
        .execute()
        .await
        .map_err(|e| format!("DB connect error: {}", e))?;

    let mut db_lock = state
        .db
        .lock()
        .map_err(|_| "Mutex lock poisoned".to_string())?;
    *db_lock = Some(conn);

    let _ = state.get_model()?;

    Ok("Connected to LanceDB successfully".to_string())
}

#[tauri::command]
pub async fn ingest_document(_path: String) -> Result<String, String>
{
    Ok("Document ingested successfully (stub)".to_string())
}

use crate::mcp::protocol::McpToolResult;

pub type RagResult = McpToolResult;

#[tauri::command]
pub async fn query_nw_db(
    query: String,
    filter: Option<String>,
    state: tauri::State<'_, RagState>,
    app: tauri::AppHandle,
) -> Result<RagResult, String>
{
    // Check registered device info first
    if let Some(info) = vendor::check_registered_device(&query, &app)
    {
        return Ok(RagResult {
            success: true,
            output: info,
        });
    }

    // Parse vendor-specific context & brand filters (resolving registered devices to vendor)
    let vendor_context = vendor::parse_vendor_context_with_app(&query, &app);
    let final_filter = vendor_context.brand_filter.or(filter);

    let db = state.get_db(&app).await?;
    let table = db
        .open_table("documents")
        .execute()
        .await
        .map_err(|e| format!("Failed to open table: {}", e))?;

    // 1. ベクトル検索（E5 Embeddings）をプライマリ検索として実行
    let model = state.get_model()?;
    let vector_searcher = VectorSearcher::new(model, vendor::get_vector_search_instruction());
    let mut batches = vector_searcher
        .search(&table, &vendor_context.query, final_filter.as_deref(), 3)
        .await?;

    // 2. ベクトル検索で有効な結果が得られなかった場合、全文検索（FTS: Full-Text Search）をフォールバックとして実行
    if !has_valid_results(&batches)
    {
        let fts_searcher = FullTextSearcher::new();
        batches = fts_searcher
            .search(&table, &vendor_context.query, final_filter.as_deref(), 3)
            .await?;
    }

    format_search_results(batches)
}

fn has_valid_results(batches: &[RecordBatch]) -> bool
{
    for batch in batches
    {
        let dist_col = batch.column_by_name("_distance");
        let dist_array = dist_col.and_then(|col| col.as_any().downcast_ref::<Float32Array>());

        for i in 0..batch.num_rows()
        {
            let distance = dist_array.map(|arr| arr.value(i)).unwrap_or(0.0);
            if distance <= 1.2
            {
                return true;
            }
        }
    }
    false
}

fn format_search_results(batches: Vec<RecordBatch>) -> Result<RagResult, String>
{
    let mut context = String::new();
    let mut count = 1;

    for batch in batches
    {
        let text_col = batch
            .column_by_name("text")
            .ok_or("Column 'text' not found in results")?;

        let path_col = batch
            .column_by_name("path")
            .ok_or("Column 'path' not found in results")?;

        let dist_col = batch.column_by_name("_distance");

        let (text_large, text_small) = (
            text_col.as_any().downcast_ref::<LargeStringArray>(),
            text_col.as_any().downcast_ref::<StringArray>(),
        );

        let (path_large, path_small) = (
            path_col.as_any().downcast_ref::<LargeStringArray>(),
            path_col.as_any().downcast_ref::<StringArray>(),
        );

        let dist_array = dist_col.and_then(|col| col.as_any().downcast_ref::<Float32Array>());

        for i in 0..batch.num_rows()
        {
            let distance = dist_array.map(|arr| arr.value(i)).unwrap_or(0.0);

            // L2 distance threshold (e.g. 1.2). If distance is larger than 1.2, the result is considered irrelevant.
            if distance > 1.2
            {
                log::info!(
                    "Skipping search result due to low similarity (distance: {})",
                    distance
                );
                continue;
            }

            let text = if let Some(arr) = text_large
            {
                arr.value(i)
            }
            else if let Some(arr) = text_small
            {
                arr.value(i)
            }
            else
            {
                return Err(format!(
                    "Failed to downcast text column. Actual type: {:?}",
                    text_col.data_type()
                ));
            };

            let path = if let Some(arr) = path_large
            {
                arr.value(i)
            }
            else if let Some(arr) = path_small
            {
                arr.value(i)
            }
            else
            {
                return Err(format!(
                    "Failed to downcast path column. Actual type: {:?}",
                    path_col.data_type()
                ));
            };

            let score = (1.0 - distance).max(0.0);
            context.push_str(&format!(
                "\n--- 検索結果 {} (ソース: {}, 類似度スコア: {:.2}) ---\n{}\n",
                count, path, score, text
            ));
            count += 1;
        }
    }

    if context.is_empty()
    {
        Ok(RagResult {
            success: true,
            output: "LanceDBに該当する情報が見つかりませんでした。".to_string(),
        })
    }
    else
    {
        Ok(RagResult {
            success: true,
            output: context,
        })
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_rag_result_serialization()
    {
        let result = RagResult {
            success: true,
            output: "RAG search results...".to_string(),
        };
        let serialized = serde_json::to_string(&result).unwrap();
        assert_eq!(
            serialized,
            r#"{"success":true,"output":"RAG search results..."}"#
        );
    }

    #[test]
    fn test_rag_state_instantiation()
    {
        let state = RagState::new();
        assert!(state.db.lock().unwrap().is_none());
        assert!(state.model.lock().unwrap().is_none());
    }

    #[test]
    fn test_has_valid_results()
    {
        use arrow::datatypes::{DataType, Field, Schema};
        use std::sync::Arc;

        // Empty batches
        assert!(!has_valid_results(&[]));

        let schema = Arc::new(Schema::new(vec![
            Field::new("text", DataType::Utf8, false),
            Field::new("path", DataType::Utf8, false),
            Field::new("_distance", DataType::Float32, false),
        ]));

        // Batch with distance <= 1.2 (valid)
        let batch_valid = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(vec!["sample text"])),
                Arc::new(StringArray::from(vec!["sample path"])),
                Arc::new(Float32Array::from(vec![0.8])),
            ],
        )
        .unwrap();
        assert!(has_valid_results(&[batch_valid]));

        // Batch with distance > 1.2 (invalid / low similarity)
        let batch_invalid = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(vec!["sample text"])),
                Arc::new(StringArray::from(vec!["sample path"])),
                Arc::new(Float32Array::from(vec![1.5])),
            ],
        )
        .unwrap();
        assert!(!has_valid_results(&[batch_invalid]));

        // Batch without distance column (e.g. FTS result)
        let schema_no_dist = Arc::new(Schema::new(vec![
            Field::new("text", DataType::Utf8, false),
            Field::new("path", DataType::Utf8, false),
        ]));
        let batch_no_dist = RecordBatch::try_new(
            schema_no_dist,
            vec![
                Arc::new(StringArray::from(vec!["sample text"])),
                Arc::new(StringArray::from(vec!["sample path"])),
            ],
        )
        .unwrap();
        assert!(has_valid_results(&[batch_no_dist]));
    }
}
