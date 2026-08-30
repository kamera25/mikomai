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
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::Manager;

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
        let mut model_lock = self
            .model
            .lock()
            .map_err(|_| "Mutex lock poisoned".to_string())?;
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
            let db_lock = self
                .db
                .lock()
                .map_err(|_| "Mutex lock poisoned".to_string())?;
            if let Some(conn) = &*db_lock {
                return Ok(conn.clone());
            }
        }

        let db_path = if let Ok(settings) = crate::settings::load_settings(app.clone()) {
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
        } else {
            let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            app_data_dir.join("lancedb").to_string_lossy().to_string()
        };

        let lancedb_dir = std::path::PathBuf::from(&db_path);

        if !lancedb_dir.exists() {
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

impl Default for RagState {
    fn default() -> Self {
        Self::new()
    }
}

#[tauri::command]
pub async fn connect_db(path: String, state: tauri::State<'_, RagState>) -> Result<String, String> {
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
pub async fn ingest_document(_path: String) -> Result<String, String> {
    Ok("Document ingested successfully (stub)".to_string())
}

use crate::mcp::protocol::McpToolResult;

pub type RagResult = McpToolResult;

/// Stable provenance for a retrieved knowledge chunk. Answers that rely on
/// RAG must expose this information to the user before it can inform changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RagCitation {
    pub source_path: String,
    pub similarity_score: f32,
    pub rank: usize,
}

#[tauri::command]
pub async fn query_nw_db(
    query: String,
    filter: Option<String>,
    state: tauri::State<'_, RagState>,
    app: tauri::AppHandle,
) -> Result<RagResult, String> {
    // Check registered device info first
    if let Some(info) = vendor::check_registered_device(&query, &app) {
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

    // Retrieve broadly from both indexes.  A vector-only fallback loses exact
    // command names, while an FTS-only fallback loses paraphrases in Japanese.
    let model = state.get_model()?;
    let vector_searcher = VectorSearcher::new(model, vendor::get_vector_search_instruction());
    let vector_batches = vector_searcher
        .search(&table, &vendor_context.query, final_filter.as_deref(), 12)
        .await?;
    let fts_batches = FullTextSearcher::new()
        .search(&table, &vendor_context.query, final_filter.as_deref(), 12)
        .await?;

    format_hybrid_search_results(vector_batches, fts_batches, &vendor_context.query)
}



#[derive(Debug, Clone)]
struct RetrievedChunk {
    path: String,
    text: String,
    vector_distance: Option<f32>,
    vector_rank: Option<usize>,
    fts_rank: Option<usize>,
}

const MAX_VECTOR_DISTANCE: f32 = 0.85;
const MIN_RERANK_SCORE: f32 = 0.32;
const RRF_K: f32 = 60.0;

fn read_string_column(batch: &RecordBatch, name: &str, row: usize) -> Result<String, String> {
    let column = batch
        .column_by_name(name)
        .ok_or_else(|| format!("Column '{}' not found in results", name))?;
    if let Some(values) = column.as_any().downcast_ref::<LargeStringArray>() {
        Ok(values.value(row).to_string())
    } else if let Some(values) = column.as_any().downcast_ref::<StringArray>() {
        Ok(values.value(row).to_string())
    } else {
        Err(format!(
            "Column '{}' is not a string (actual: {:?})",
            name,
            column.data_type()
        ))
    }
}

fn collect_candidates(
    batches: Vec<RecordBatch>,
    is_vector: bool,
    candidates: &mut HashMap<String, RetrievedChunk>,
) -> Result<(), String> {
    let mut rank = 1;
    for batch in batches {
        let distances = batch
            .column_by_name("_distance")
            .and_then(|column| column.as_any().downcast_ref::<Float32Array>());
        for row in 0..batch.num_rows() {
            let path = read_string_column(&batch, "path", row)?;
            let text = read_string_column(&batch, "text", row)?;
            let key = format!("{}\u{0}{}", path, text);
            let candidate = candidates.entry(key).or_insert_with(|| RetrievedChunk {
                path: path.clone(),
                text: text.clone(),
                vector_distance: None,
                vector_rank: None,
                fts_rank: None,
            });
            if is_vector {
                candidate.vector_distance = distances.map(|values| values.value(row));
                candidate.vector_rank = Some(rank);
            } else {
                candidate.fts_rank = Some(rank);
            }
            rank += 1;
        }
    }
    Ok(())
}

fn lexical_overlap(query: &str, text: &str) -> f32 {
    let terms: Vec<_> = query
        .split_whitespace()
        .map(|term| {
            term.trim_matches(|c: char| {
                !c.is_alphanumeric()
                    && !('\u{3040}'..='\u{30ff}').contains(&c)
                    && !('\u{4e00}'..='\u{9fff}').contains(&c)
            })
        })
        .filter(|term| term.chars().count() >= 2)
        .collect();
    if terms.is_empty() {
        return 0.0;
    }
    let text = text.to_lowercase();
    let matches = terms
        .iter()
        .filter(|term| text.contains(&term.to_lowercase()))
        .count();
    matches as f32 / terms.len() as f32
}

fn rerank_score(candidate: &RetrievedChunk, query: &str) -> f32 {
    let semantic = candidate
        .vector_distance
        .map(|distance| (1.0 - distance / 1.2).clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let lexical = lexical_overlap(query, &candidate.text);
    let rrf = [candidate.vector_rank, candidate.fts_rank]
        .into_iter()
        .flatten()
        .map(|rank| RRF_K / (RRF_K + rank as f32))
        .fold(0.0_f32, f32::max);
    0.50 * semantic + 0.30 * lexical + 0.20 * rrf
}

fn is_supported(candidate: &RetrievedChunk, query: &str) -> Option<f32> {
    let lexical = lexical_overlap(query, &candidate.text);
    let vector_is_relevant = candidate
        .vector_distance
        .map(|distance| distance <= MAX_VECTOR_DISTANCE)
        .unwrap_or(false);
    let score = rerank_score(candidate, query);
    ((vector_is_relevant || lexical >= 0.5) && score >= MIN_RERANK_SCORE).then_some(score)
}

fn format_hybrid_search_results(
    vector_batches: Vec<RecordBatch>,
    fts_batches: Vec<RecordBatch>,
    query: &str,
) -> Result<RagResult, String> {
    let mut context = String::new();
    let mut candidates = HashMap::new();
    collect_candidates(vector_batches, true, &mut candidates)?;
    collect_candidates(fts_batches, false, &mut candidates)?;
    let mut ranked: Vec<_> = candidates
        .into_values()
        .filter_map(|candidate| is_supported(&candidate, query).map(|score| (candidate, score)))
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));

    // An explicit command name is stronger evidence than semantic similarity.
    // Otherwise a vendor-scoped hostname query can include unrelated VLAN
    // templates and make their parameters look required downstream.
    let query_lower = query.to_lowercase();
    if query_lower.contains("hostname") {
        let hostname_matches: Vec<_> = ranked
            .iter()
            .filter(|(candidate, _)| candidate.text.to_lowercase().contains("hostname"))
            .cloned()
            .collect();
        if !hostname_matches.is_empty() {
            ranked = hostname_matches;
        }
    }

    for (rank, (candidate, score)) in ranked.into_iter().take(5).enumerate() {
        let citation = RagCitation {
            source_path: candidate.path,
            similarity_score: score,
            rank: rank + 1,
        };
        context.push_str(&format_citation(&citation, &candidate.text));
    }

    if context.is_empty() {
        Ok(RagResult {
            success: true,
            output: "LanceDBに該当する情報が見つかりませんでした。".to_string(),
        })
    } else {
        Ok(RagResult {
            success: true,
            output: context,
        })
    }
}

fn format_citation(citation: &RagCitation, text: &str) -> String {
    format!(
        "\n--- 根拠 [{}] (ソース: {}, 類似度スコア: {:.2}) ---\n{}\n",
        citation.rank, citation.source_path, citation.similarity_score, text
    )
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
        assert_eq!(
            serialized,
            r#"{"success":true,"output":"RAG search results..."}"#
        );
    }

    #[test]
    fn test_rag_state_instantiation() {
        let state = RagState::new();
        assert!(state.db.lock().unwrap().is_none());
        assert!(state.model.lock().unwrap().is_none());
    }



    #[test]
    fn citation_keeps_source_and_rank_visible_to_callers() {
        let citation = RagCitation {
            source_path: "nw-docs/cisco/show_version.md".to_string(),
            similarity_score: 0.82,
            rank: 1,
        };
        let result = format_citation(&citation, "show version の説明");
        assert!(result.contains("根拠 [1]"));
        assert!(result.contains("nw-docs/cisco/show_version.md"));
    }

    #[test]
    fn hybrid_reranker_prefers_exact_command_over_weak_semantic_match() {
        let exact = RetrievedChunk {
            path: "exact".into(),
            text: "show ip route でルーティングを確認".into(),
            vector_distance: Some(0.7),
            vector_rank: Some(2),
            fts_rank: Some(1),
        };
        let weak = RetrievedChunk {
            path: "weak".into(),
            text: "VLAN の基本説明".into(),
            vector_distance: Some(0.8),
            vector_rank: Some(1),
            fts_rank: None,
        };
        assert!(
            rerank_score(&exact, "show ip route 確認") > rerank_score(&weak, "show ip route 確認")
        );
    }

    #[test]
    fn unsupported_result_is_not_exposed_as_evidence() {
        let candidate = RetrievedChunk {
            path: "irrelevant".into(),
            text: "VLAN の基本説明".into(),
            vector_distance: Some(1.05),
            vector_rank: Some(1),
            fts_rank: None,
        };
        assert!(is_supported(&candidate, "QuantumRouter9000 独自コマンド").is_none());
    }
}
