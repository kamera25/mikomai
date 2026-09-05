pub mod vendor;

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use serde::{Deserialize, Serialize};
use surrealdb::types::SurrealValue;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::Manager;

pub struct RagState {
    pub model: Mutex<Option<Arc<TextEmbedding>>>,
}

impl RagState {
    pub fn new() -> Self {
        Self {
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

}

impl Default for RagState {
    fn default() -> Self {
        Self::new()
    }
}

#[tauri::command]
pub async fn ingest_document(
    path: String,
    state: tauri::State<'_, RagState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let graph = app.state::<crate::graph::SurrealDbState>();
    let count = ingest_path(Path::new(&path), &state, &graph).await?;
    Ok(format!("Ingested {count} knowledge chunks into SurrealDB"))
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

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
struct RagChunk {
    path: String,
    text: String,
    brand: String,
    #[serde(default)]
    chunk_index: usize,
    #[serde(default)]
    distance: f32,
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
    // Raw query fragments are intentionally unsupported; vendor context is the
    // parameter-bound metadata filter.
    if filter.is_some_and(|value| !value.trim().is_empty()) {
        return Err("Raw RAG filters are no longer supported; use a vendor context".to_string());
    }
    let brand = vendor_context.brand_filter;

    let model = state.get_model()?;
    let instructional_query = format!(
        "Instruct: {}\nQuery: {}",
        vendor::get_vector_search_instruction(),
        vendor_context.query
    );
    let embedding = model
        .embed(vec![instructional_query], None)
        .map_err(|e| format!("Embedding error: {e}"))?
        .into_iter()
        .next()
        .ok_or("Failed to generate embedding")?;
    let graph = app.state::<crate::graph::SurrealDbState>();
    let chunks = search_chunks(&graph, embedding, brand, &vendor_context.query).await?;
    format_search_results(chunks, &vendor_context.query)
}

fn is_japanese(c: char) -> bool {
    ('\u{3040}'..='\u{30ff}').contains(&c) || ('\u{4e00}'..='\u{9fff}').contains(&c)
}

/// Produce useful lexical terms for both Japanese prose and command syntax.
/// Japanese queries are commonly written without spaces, so each Japanese run
/// also contributes two-character terms.  ASCII command tokens stay intact.
fn lexical_terms(query: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut token = String::new();
    let mut japanese_token = false;
    let flush = |token: &mut String, japanese: bool, terms: &mut Vec<String>| {
        if token.chars().count() >= 2 {
            let normalized = token.to_lowercase();
            terms.push(normalized.clone());
            if japanese {
                let chars: Vec<_> = normalized.chars().collect();
                for pair in chars.windows(2) {
                    terms.push(pair.iter().collect());
                }
            }
        }
        token.clear();
    };

    for c in query.chars() {
        let japanese = is_japanese(c);
        let is_term_char = c.is_alphanumeric() || c == '-' || c == '_' || japanese;
        if !is_term_char {
            flush(&mut token, japanese_token, &mut terms);
            japanese_token = false;
        } else {
            if !token.is_empty() && japanese != japanese_token {
                flush(&mut token, japanese_token, &mut terms);
            }
            token.push(c);
            japanese_token = japanese;
        }
    }
    flush(&mut token, japanese_token, &mut terms);
    terms.sort();
    terms.dedup();
    terms
}

fn lexical_overlap(query: &str, text: &str) -> f32 {
    let terms = lexical_terms(query);
    if terms.is_empty() {
        return 0.0;
    }
    let text = text.to_lowercase();
    let matches = terms
        .iter()
        .filter(|term| text.contains(term.as_str()))
        .count();
    matches as f32 / terms.len() as f32
}

fn rerank_score(candidate: &RagChunk, query: &str) -> f32 {
    let semantic = (1.0 - candidate.distance / 2.0).clamp(0.0, 1.0);
    let lexical = lexical_overlap(query, &candidate.text);
    0.60 * semantic + 0.40 * lexical
}

fn is_supported(candidate: &RagChunk, query: &str) -> Option<f32> {
    let lexical = lexical_overlap(query, &candidate.text);
    let score = rerank_score(candidate, query);
    ((candidate.distance <= 0.85 || lexical >= 0.5) && score >= 0.32).then_some(score)
}

fn format_search_results(chunks: Vec<RagChunk>, query: &str) -> Result<RagResult, String> {
    let mut context = String::new();
    let mut ranked: Vec<_> = chunks
        .into_iter()
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

    // Avoid spending every context slot on one long manual, while retaining a
    // neighbouring chunk when it supplies a prerequisite or warning.
    let mut chunks_per_path: HashMap<String, usize> = HashMap::new();
    let mut emitted = 0;
    for (candidate, score) in ranked.into_iter() {
        let count = chunks_per_path.entry(candidate.path.clone()).or_default();
        if *count >= 2 {
            continue;
        }
        *count += 1;
        emitted += 1;
        let citation = RagCitation {
            source_path: candidate.path,
            similarity_score: score,
            rank: emitted,
        };
        context.push_str(&format_citation(&citation, &candidate.text));
        if emitted == 5 {
            break;
        }
    }

    if context.is_empty() {
        Ok(RagResult {
            success: true,
            output: "SurrealDBに該当する情報が見つかりませんでした。".to_string(),
        })
    } else {
        Ok(RagResult {
            success: true,
            output: context,
        })
    }
}

async fn search_chunks(graph: &crate::graph::SurrealDbState, embedding: Vec<f32>, brand: Option<String>, query: &str) -> Result<Vec<RagChunk>, String> {
    let sql = if brand.is_some() {
        "SELECT path, text, brand, chunk_index, vector::distance::knn() AS distance FROM rag_chunk WHERE brand = $brand AND embedding <|30,100|> $embedding LIMIT 30;"
    } else {
        "SELECT path, text, brand, chunk_index, vector::distance::knn() AS distance FROM rag_chunk WHERE embedding <|30,100|> $embedding LIMIT 30;"
    };
    let mut response = graph.db.query(sql).bind(("embedding", embedding)).bind(("brand", brand.clone().unwrap_or_default())).await
        .map_err(|e| format!("SurrealDB vector search failed: {e}"))?;
    let mut chunks: Vec<RagChunk> = response.take(0)
        .map_err(|e| format!("Failed to decode SurrealDB vector search results: {e}"))?;

    // FTS supplies exact command-name candidates; the post-query lexical
    // reranker below prevents a weak keyword hit from becoming evidence.
    let lexical_query = lexical_terms(query).join(" ");
    if !lexical_query.is_empty() {
        let sql = if brand.is_some() {
            "SELECT path, text, brand, chunk_index, 2.0 AS distance FROM rag_chunk WHERE brand = $brand AND text @1@ $query LIMIT 30;"
        } else {
            "SELECT path, text, brand, chunk_index, 2.0 AS distance FROM rag_chunk WHERE text @1@ $query LIMIT 30;"
        };
        let mut response = graph.db.query(sql).bind(("brand", brand.unwrap_or_default())).bind(("query", lexical_query)).await
            .map_err(|e| format!("SurrealDB full-text search failed: {e}"))?;
        let lexical: Vec<RagChunk> = response.take(0)
            .map_err(|e| format!("Failed to decode SurrealDB full-text search results: {e}"))?;
        let mut seen: std::collections::HashSet<_> = chunks.iter().map(|chunk| (chunk.path.clone(), chunk.chunk_index)).collect();
        chunks.extend(lexical.into_iter().filter(|chunk| seen.insert((chunk.path.clone(), chunk.chunk_index))));
    }
    Ok(chunks)
}

pub(crate) async fn ingest_path(path: &Path, state: &RagState, graph: &crate::graph::SurrealDbState) -> Result<usize, String> {
    let mut files = Vec::new();
    collect_markdown_files(path, &mut files)?;
    let model = state.get_model()?;
    let mut count = 0;
    for file in files {
        let raw = fs::read_to_string(&file).map_err(|e| format!("Failed to read {}: {e}", file.display()))?;
        let (metadata, content) = parse_frontmatter(&raw);
        let chunks = split_chunks(&replace_metadata_placeholders(content, &metadata), 1400, 180)?;
        let embeddings = model.embed(chunks.iter().map(|chunk| format!("passage: {chunk}")).collect(), None)
            .map_err(|e| format!("Embedding {} failed: {e}", file.display()))?;
        let path = file.to_string_lossy().to_string();
        let brand = metadata.get("brand")
            .map(|value| crate::mcp::brands::get_brand(value).unwrap_or(value).to_string())
            .unwrap_or_default();
        graph.db.query("DELETE rag_chunk WHERE path = $path;").bind(("path", path.clone())).await
            .map_err(|e| format!("Failed to replace existing chunks for {path}: {e}"))?;
        for (index, (text, embedding)) in chunks.into_iter().zip(embeddings).enumerate() {
            let id = stable_id(&format!("{path}:{index}"));
            graph.db.query("UPSERT type::record('rag_chunk', $id) CONTENT $record;")
                .bind(("id", id)).bind(("record", serde_json::json!({
                    "path": path, "text": text, "brand": brand,
                    "os_version": metadata.get("os_version").cloned().unwrap_or_default(),
                    "category": metadata.get("category").cloned().unwrap_or_default(),
                    "command_type": metadata.get("command_type").cloned().unwrap_or_default(),
                    "target_model": metadata.get("target_model").cloned().unwrap_or_default(),
                    "chunk_index": index, "embedding": embedding,
                }))).await.map_err(|e| format!("Failed to ingest {path}: {e}"))?;
            count += 1;
        }
    }
    Ok(count)
}

fn collect_markdown_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_file() {
        if path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("md")) { files.push(path.to_path_buf()); }
        return Ok(());
    }
    for entry in fs::read_dir(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))? {
        collect_markdown_files(&entry.map_err(|e| e.to_string())?.path(), files)?;
    }
    Ok(())
}

fn parse_frontmatter(raw: &str) -> (HashMap<String, String>, &str) {
    let Some(rest) = raw.strip_prefix("---\n") else { return (HashMap::new(), raw); };
    let Some(end) = rest.find("\n---\n") else { return (HashMap::new(), raw); };
    let metadata = rest[..end].lines().filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned())).collect();
    (metadata, &rest[end + 5..])
}

fn replace_metadata_placeholders(content: &str, metadata: &HashMap<String, String>) -> String {
    metadata.iter().fold(content.to_owned(), |result, (key, value)| result.replace(&format!("{{{key}}}"), value))
}

fn split_chunks(content: &str, chunk_size: usize, overlap: usize) -> Result<Vec<String>, String> {
    if chunk_size == 0 || overlap >= chunk_size { return Err("Invalid RAG chunk size configuration".to_string()); }
    let mut chunks = Vec::new();
    for section in content.split("\n#").filter(|section| !section.trim().is_empty()) {
        let section = if content.starts_with(section) { section.to_owned() } else { format!("#{section}") };
        if section.len() <= chunk_size { chunks.push(section.trim().to_owned()); continue; }
        let mut start = 0;
        while start < section.len() {
            let mut end = (start + chunk_size).min(section.len());
            while end > start && !section.is_char_boundary(end) { end -= 1; }
            if end < section.len() { if let Some(boundary) = section[start..end].rfind('\n').filter(|boundary| *boundary > chunk_size / 2) { end = start + boundary + 1; } }
            chunks.push(section[start..end].trim().to_owned());
            if end == section.len() { break; }
            start = end.saturating_sub(overlap);
        }
    }
    Ok(chunks)
}

fn stable_id(value: &str) -> String {
    let hash = value.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3));
    format!("{hash:016x}")
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
    fn reranker_prefers_exact_command_over_weak_semantic_match() {
        let exact = RagChunk {
            path: "exact".into(),
            text: "show ip route でルーティングを確認".into(),
            brand: "Cisco".into(), chunk_index: 0, distance: 0.7,
        };
        let weak = RagChunk {
            path: "weak".into(),
            text: "VLAN の基本説明".into(),
            brand: "Cisco".into(), chunk_index: 0, distance: 0.8,
        };
        assert!(
            rerank_score(&exact, "show ip route 確認") > rerank_score(&weak, "show ip route 確認")
        );
    }

    #[test]
    fn unsupported_result_is_not_exposed_as_evidence() {
        let candidate = RagChunk {
            path: "irrelevant".into(),
            text: "VLAN の基本説明".into(),
            brand: "Cisco".into(), chunk_index: 0, distance: 1.05,
        };
        assert!(is_supported(&candidate, "QuantumRouter9000 独自コマンド").is_none());
    }

    #[test]
    fn lexical_overlap_handles_japanese_without_spaces() {
        assert!(
            lexical_overlap(
                "ヤマハのルーティングテーブル確認",
                "IPルーティングテーブルを表示します"
            ) > 0.3
        );
    }

    #[test]
    fn lexical_terms_preserve_command_tokens() {
        let terms = lexical_terms("show ip route で経路を確認");
        assert!(terms.contains(&"show".to_string()));
        assert!(terms.contains(&"route".to_string()));
        assert!(terms.contains(&"経路".to_string()));
    }

    #[test]
    fn frontmatter_and_chunks_keep_vendor_context() {
        let (metadata, content) = parse_frontmatter("---\nbrand: Cisco\n---\n# {brand} command");
        assert_eq!(replace_metadata_placeholders(content, &metadata), "# Cisco command");
        assert_eq!(split_chunks("# one\nbody\n# two\nbody", 1400, 180).unwrap().len(), 2);
    }

    #[tokio::test]
    async fn surreal_vector_search_returns_bound_metadata() {
        let path = std::env::temp_dir().join(format!("mikomai-rag-test-{}", uuid::Uuid::new_v4()));
        let graph = crate::graph::SurrealDbState::initialize_at(&path).await.unwrap();
        let mut embedding = vec![0.0_f32; 1024];
        embedding[0] = 1.0;
        graph.db.query("CREATE rag_chunk:test CONTENT { path: 'manual.md', text: 'show ip route', brand: 'cisco_ios', chunk_index: 0, embedding: $embedding };")
            .bind(("embedding", embedding.clone())).await.unwrap();
        let results = search_chunks(&graph, embedding, Some("cisco_ios".to_string()), "show ip route").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "manual.md");
        std::fs::remove_dir_all(path).unwrap();
    }
}
