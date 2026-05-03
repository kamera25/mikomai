use lancedb::connection::Connection;
use lancedb::connect;
use lancedb::query::{ExecutableQuery, QueryBase};
use std::sync::{Arc, Mutex};
use fastembed::{TextEmbedding, EmbeddingModel, InitOptions};
use futures::StreamExt;
use arrow_array::{RecordBatch, StringArray};

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

        println!("Initializing embedding model (all-MiniLM-L6-v2)...");
        let mut options = InitOptions::default();
        options.model_name = EmbeddingModel::AllMiniLML6V2;
        options.show_download_progress = true;
        
        let model = TextEmbedding::try_new(options)
            .map_err(|e| format!("Failed to initialize embedding model: {}", e))?;

        let arc_model = Arc::new(model);
        *model_lock = Some(arc_model.clone());
        Ok(arc_model)
    }
}

#[tauri::command]
pub async fn connect_db(path: String, state: tauri::State<'_, RagState>) -> Result<String, String> {
    println!("Connecting to LanceDB at: {}", path);
    let conn = connect(&path).execute().await.map_err(|e| format!("DB connect error: {}", e))?;
    
    let mut db_lock = state.db.lock().unwrap();
    *db_lock = Some(conn);
    
    // Pre-initialize the model to avoid delay on first query
    let _ = state.get_model()?;
    
    Ok("Connected to LanceDB successfully".to_string())
}

#[tauri::command]
pub async fn ingest_document(path: String) -> Result<String, String> {
    // This is a stub for the document ingestion pipeline.
    println!("Ingesting document from: {}", path);
    Ok("Document ingested successfully (stub)".to_string())
}

#[tauri::command]
pub async fn query_rag(query: String, filter: Option<String>, state: tauri::State<'_, RagState>) -> Result<String, String> {
    println!("Querying RAG (Rust-native): {} (filter: {:?})", query, filter);

    // 1. Get DB connection
    let db = {
        let db_lock = state.db.lock().unwrap();
        db_lock.as_ref().ok_or("Database not connected")?.clone()
    };

    // 2. Get/Init embedding model
    let model = state.get_model()?;

    // 3. Generate embedding for the query
    let embeddings = model.embed(vec![query], None)
        .map_err(|e| format!("Embedding error: {}", e))?;
    let query_vector = embeddings.first().ok_or("Failed to generate embedding")?.clone();

    // 4. Perform search in LanceDB
    let table = db.open_table("documents").execute().await
        .map_err(|e| format!("Failed to open table: {}", e))?;

    // Use the correct Query API for LanceDB Rust SDK 0.27.2
    let mut vector_query = table.query()
        .nearest_to(query_vector)
        .map_err(|e| format!("Vector search error: {}", e))?
        .limit(3);
    
    if let Some(filter_str) = filter {
        // In lancedb-rs 0.27.2, VectorQuery uses only_if for filtering
        vector_query = vector_query.only_if(filter_str);
    }

    let mut stream = vector_query.execute().await
        .map_err(|e| format!("Search execution error: {}", e))?;

    let mut context = String::new();
    let mut count = 1;

    while let Some(batch_result) = stream.next().await {
        let batch: RecordBatch = batch_result.map_err(|e| format!("Error reading search results: {}", e))?;
        
        // Extract text and path columns
        let text_col = batch.column_by_name("text")
            .ok_or("Column 'text' not found in results")?
            .as_any().downcast_ref::<StringArray>()
            .ok_or("Failed to downcast text column")?;
            
        let path_col = batch.column_by_name("path")
            .ok_or("Column 'path' not found in results")?
            .as_any().downcast_ref::<StringArray>()
            .ok_or("Failed to downcast path column")?;

        for i in 0..batch.num_rows() {
            let text = text_col.value(i);
            let path = path_col.value(i);
            context.push_str(&format!("\n--- Result {} (Source: {}) ---\n{}\n", count, path, text));
            count += 1;
        }
    }

    if context.is_empty() {
        Ok("No relevant information found in LanceDB.".to_string())
    } else {
        Ok(context)
    }
}
