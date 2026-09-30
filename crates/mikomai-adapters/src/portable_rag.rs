//! Portable vector RAG over the `rag_chunk` table owned by `PortableGraph`.
//!
//! The platform supplies the embedding model; this module owns the legacy
//! chunking, multilingual retrieval, reranking, citation, preview, and bounded
//! document-expansion behavior.

use crate::portable_graph::{PortableGraph, RagChunkCandidate, RagChunkRecord};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

const EMBEDDING_DIMENSION: usize = 1024;
const VECTOR_INSTRUCTION: &str =
    "ネットワーク機器の操作マニュアルから、関連する設定コマンドや手順を検索します。";

/// Host-provided E5 embedding implementation. Inputs are raw text; this
/// module applies the legacy `passage:` and E5 query instruction prefixes.
pub trait RagEmbedder: Send + Sync {
    fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, String>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RagCitation {
    pub source_path: String,
    pub similarity_score: f32,
    pub rank: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagSearchResult {
    pub success: bool,
    pub output: String,
    pub citations: Vec<RagCitation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagDocumentPreview {
    pub path: String,
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Clone)]
struct RankedChunk {
    candidate: RagChunkCandidate,
    score: f32,
}

/// RAG orchestration using the graph's existing embedded database handle.
/// Cloning `PortableGraph` clones its Surreal handle; it does not create a
/// second database owner or select another storage path.
pub struct PortableRag {
    graph: PortableGraph,
    embedder: Arc<dyn RagEmbedder>,
}

impl PortableRag {
    pub fn new(graph: PortableGraph, embedder: Arc<dyn RagEmbedder>) -> Self {
        Self { graph, embedder }
    }

    /// Ingest one Markdown file or recursively ingest Markdown files below a
    /// directory. Each source replaces its prior chunks by stable path/index ID.
    pub async fn ingest_path(&self, path: &Path) -> Result<usize, String> {
        let mut files = Vec::new();
        collect_markdown_files(path, &mut files)?;
        files.sort();
        let mut count = 0;
        for file in files {
            let raw = fs::read_to_string(&file)
                .map_err(|e| format!("Failed to read {}: {e}", file.display()))?;
            let (metadata, content) = parse_frontmatter(&raw);
            let document = replace_metadata_placeholders(content, &metadata);
            let title = document_title(&document, &file);
            let summary = summarize_text(&document);
            let chunks = split_chunks(&document, 1400, 180)?;
            let inputs: Vec<_> = chunks
                .iter()
                .map(|chunk| format!("passage: {chunk}"))
                .collect();
            let embeddings = if inputs.is_empty() {
                Vec::new()
            } else {
                self.embedder
                    .embed(&inputs)
                    .map_err(|e| format!("Embedding {} failed: {e}", file.display()))?
            };
            validate_embedding_batch(&inputs, &embeddings, &file.display().to_string())?;
            let source_path = file.to_string_lossy().to_string();
            let brand = metadata
                .get("brand")
                .map(|value| normalize_brand(value).unwrap_or_else(|| value.trim().to_owned()))
                .unwrap_or_default();
            let records: Vec<_> = chunks
                .into_iter()
                .zip(embeddings)
                .enumerate()
                .map(|(chunk_index, (text, embedding))| RagChunkRecord {
                    path: source_path.clone(),
                    text,
                    brand: brand.clone(),
                    title: title.clone(),
                    summary: summary.clone(),
                    os_version: metadata.get("os_version").cloned().unwrap_or_default(),
                    category: metadata.get("category").cloned().unwrap_or_default(),
                    command_type: metadata.get("command_type").cloned().unwrap_or_default(),
                    target_model: metadata.get("target_model").cloned().unwrap_or_default(),
                    chunk_index,
                    embedding,
                })
                .collect();
            self.graph
                .replace_rag_document(&source_path, &records)
                .await?;
            count += records.len();
        }
        Ok(count)
    }

    /// Search vector-nearest chunks, supplement with exact full-text matches,
    /// then apply the previous semantic/lexical evidence thresholds and
    /// citation format. `query` should already have vendor-context tags
    /// removed and any registered-device vendor resolved by the host.
    pub async fn search(
        &self,
        query: &str,
        brand_filter: Option<&str>,
    ) -> Result<RagSearchResult, String> {
        if query.trim().is_empty() {
            return Err("NW-DB search query is required".to_owned());
        }
        let brand = match brand_filter.filter(|value| !value.trim().is_empty()) {
            Some(value) => Some(normalize_brand(value).ok_or_else(|| {
                "Raw RAG filters are no longer supported; use a vendor context".to_owned()
            })?),
            None => None,
        };

        let instruction_query = format!("Instruct: {VECTOR_INSTRUCTION}\nQuery: {query}");
        let inputs = vec![instruction_query];
        let mut embeddings = self.embedder.embed(&inputs)?;
        validate_embedding_batch(&inputs, &embeddings, "query")?;
        let embedding = embeddings.pop().expect("batch length validated");
        let mut candidates = self
            .graph
            .search_rag_vectors(&embedding, brand.as_deref())
            .await?;
        let terms = lexical_terms(query).join(" ");
        if !terms.is_empty() {
            let lexical = self
                .graph
                .search_rag_lexical(&terms, brand.as_deref())
                .await?;
            let mut seen: HashSet<_> = candidates
                .iter()
                .map(|candidate| (candidate.path.clone(), candidate.chunk_index))
                .collect();
            candidates.extend(
                lexical.into_iter().filter(|candidate| {
                    seen.insert((candidate.path.clone(), candidate.chunk_index))
                }),
            );
        }
        Ok(format_search_results(candidates, query))
    }

    /// Resolve only cited titles and summaries before an LLM chooses sources.
    pub async fn previews_for_search_result(
        &self,
        result: &RagSearchResult,
    ) -> Result<Vec<RagDocumentPreview>, String> {
        let mut previews = Vec::new();
        let mut seen = HashSet::new();
        for citation in result.citations.iter().take(5) {
            if !seen.insert(citation.source_path.clone()) {
                continue;
            }
            let chunks = self
                .graph
                .rag_document_chunks(&citation.source_path)
                .await?;
            if let Some(row) = chunks.into_iter().next() {
                previews.push(RagDocumentPreview {
                    path: row.path.clone(),
                    title: if row.title.is_empty() {
                        row.path
                    } else {
                        row.title
                    },
                    summary: if row.summary.is_empty() {
                        summarize_text(&row.text)
                    } else {
                        row.summary
                    },
                });
            }
        }
        Ok(previews)
    }

    /// Expand at most three selected sources and cap the total context at
    /// 18,000 Unicode characters, matching the prior manual-selection flow.
    pub async fn expand_selected_documents(&self, paths: &[String]) -> Result<String, String> {
        const MAX_DOCUMENTS: usize = 3;
        const MAX_CONTEXT_CHARS: usize = 18_000;
        let mut expanded = String::new();
        for path in paths.iter().take(MAX_DOCUMENTS) {
            let rows = self.graph.rag_document_chunks(path).await?;
            if rows.is_empty() {
                continue;
            }
            expanded.push_str(&format!(
                "\n\n=== 選択資料: {} ({}) ===\n",
                rows[0].title, path
            ));
            for row in rows {
                if expanded.chars().count() >= MAX_CONTEXT_CHARS {
                    break;
                }
                expanded.push_str(&row.text);
                expanded.push('\n');
            }
            if expanded.chars().count() >= MAX_CONTEXT_CHARS {
                break;
            }
        }
        if expanded.chars().count() > MAX_CONTEXT_CHARS {
            expanded = expanded.chars().take(MAX_CONTEXT_CHARS).collect();
        }
        Ok(expanded)
    }
}

fn validate_embedding_batch(
    inputs: &[String],
    outputs: &[Vec<f32>],
    label: &str,
) -> Result<(), String> {
    if inputs.len() != outputs.len() {
        return Err(format!(
            "Embedding {label} returned {} vectors for {} inputs",
            outputs.len(),
            inputs.len()
        ));
    }
    if let Some((index, vector)) = outputs
        .iter()
        .enumerate()
        .find(|(_, vector)| vector.len() != EMBEDDING_DIMENSION)
    {
        return Err(format!(
            "Embedding {label} vector {index} must contain {EMBEDDING_DIMENSION} values (got {})",
            vector.len()
        ));
    }
    Ok(())
}

fn format_search_results(candidates: Vec<RagChunkCandidate>, query: &str) -> RagSearchResult {
    let mut ranked: Vec<_> = candidates
        .into_iter()
        .filter_map(|candidate| {
            evidence_score(&candidate, query).map(|score| RankedChunk { candidate, score })
        })
        .collect();
    ranked.sort_by(|left, right| right.score.total_cmp(&left.score));

    if query.to_lowercase().contains("hostname") {
        let hostname: Vec<_> = ranked
            .iter()
            .filter(|item| item.candidate.text.to_lowercase().contains("hostname"))
            .cloned()
            .collect();
        if !hostname.is_empty() {
            ranked = hostname;
        }
    }

    let mut output = String::new();
    let mut citations = Vec::new();
    let mut chunks_per_path: HashMap<String, usize> = HashMap::new();
    for item in ranked {
        let count = chunks_per_path
            .entry(item.candidate.path.clone())
            .or_default();
        if *count >= 2 {
            continue;
        }
        *count += 1;
        let citation = RagCitation {
            source_path: item.candidate.path,
            similarity_score: item.score,
            rank: citations.len() + 1,
        };
        output.push_str(&format_citation(&citation, &item.candidate.text));
        citations.push(citation);
        if citations.len() == 5 {
            break;
        }
    }

    if output.is_empty() {
        output = "SurrealDBに該当する情報が見つかりませんでした。".to_owned();
    }
    RagSearchResult {
        success: true,
        output,
        citations,
    }
}

fn evidence_score(candidate: &RagChunkCandidate, query: &str) -> Option<f32> {
    let lexical = lexical_overlap(query, &candidate.text);
    let semantic = (1.0 - candidate.distance / 2.0).clamp(0.0, 1.0);
    let score = 0.60 * semantic + 0.40 * lexical;
    ((candidate.distance <= 0.85 || lexical >= 0.5) && score >= 0.32).then_some(score)
}

fn format_citation(citation: &RagCitation, text: &str) -> String {
    format!(
        "\n--- 根拠 [{}] (ソース: {}, 類似度スコア: {:.2}) ---\n{}\n",
        citation.rank, citation.source_path, citation.similarity_score, text
    )
}

fn is_japanese(c: char) -> bool {
    ('\u{3040}'..='\u{30ff}').contains(&c) || ('\u{4e00}'..='\u{9fff}').contains(&c)
}

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
        let term_char = c.is_alphanumeric() || c == '-' || c == '_' || japanese;
        if !term_char {
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
    terms
        .iter()
        .filter(|term| text.contains(term.as_str()))
        .count() as f32
        / terms.len() as f32
}

fn collect_markdown_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_file() {
        if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }
    for entry in
        fs::read_dir(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?
    {
        collect_markdown_files(&entry.map_err(|e| e.to_string())?.path(), files)?;
    }
    Ok(())
}

fn parse_frontmatter(raw: &str) -> (HashMap<String, String>, &str) {
    let Some(rest) = raw.strip_prefix("---\n") else {
        return (HashMap::new(), raw);
    };
    let Some(end) = rest.find("\n---\n") else {
        return (HashMap::new(), raw);
    };
    let metadata = rest[..end]
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
        .collect();
    (metadata, &rest[end + 5..])
}

fn replace_metadata_placeholders(content: &str, metadata: &HashMap<String, String>) -> String {
    metadata
        .iter()
        .fold(content.to_owned(), |result, (key, value)| {
            result.replace(&format!("{{{key}}}"), value)
        })
}

fn document_title(document: &str, path: &Path) -> String {
    document
        .lines()
        .find_map(|line| line.trim().strip_prefix('#').map(str::trim))
        .filter(|title| !title.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("Untitled")
                .to_owned()
        })
}

fn summarize_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(240)
        .collect()
}

fn split_chunks(content: &str, chunk_size: usize, overlap: usize) -> Result<Vec<String>, String> {
    if chunk_size == 0 || overlap >= chunk_size {
        return Err("Invalid RAG chunk size configuration".to_owned());
    }
    let mut chunks = Vec::new();
    for section in content
        .split("\n#")
        .filter(|section| !section.trim().is_empty())
    {
        let section = if content.starts_with(section) {
            section.to_owned()
        } else {
            format!("#{section}")
        };
        if section.len() <= chunk_size {
            chunks.push(section.trim().to_owned());
            continue;
        }
        let mut start = 0;
        while start < section.len() {
            let mut end = (start + chunk_size).min(section.len());
            while end > start && !section.is_char_boundary(end) {
                end -= 1;
            }
            if end < section.len() {
                if let Some(boundary) = section[start..end]
                    .rfind('\n')
                    .filter(|boundary| *boundary > chunk_size / 2)
                {
                    end = start + boundary + 1;
                }
            }
            chunks.push(section[start..end].trim().to_owned());
            if end == section.len() {
                break;
            }
            start = end.saturating_sub(overlap);
        }
    }
    Ok(chunks)
}

fn normalize_brand(input: &str) -> Option<String> {
    let normalized = input.trim().to_lowercase().replace([' ', '-', '/'], "_");
    let brand = match normalized.as_str() {
        "cisco" | "cisco_ios" | "cisco_xe" | "cisco_xr" | "cisco_nxos" | "ios" => "cisco_ios",
        "juniper" | "junos" | "juniper_junos" => "juniper_junos",
        "arista" | "eos" | "arista_eos" => "arista_eos",
        "yamaha" => "yamaha",
        "furukawa" | "fitelnet" | "furukawa_fitelnet" => "furukawa_fitelnet",
        "fortinet" | "fortigate" | "fortios" => "fortinet",
        "a10" | "a10_ax" | "a10_vthreads" => "a10",
        "paloalto" | "palo_alto" | "panos" | "paloalto_panos" => "paloalto_panos",
        _ => return None,
    };
    Some(brand.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestEmbedder;

    impl RagEmbedder for TestEmbedder {
        fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, String> {
            Ok(inputs
                .iter()
                .map(|input| {
                    let mut vector = vec![0.0; EMBEDDING_DIMENSION];
                    vector[if input.to_lowercase().contains("cisco") {
                        0
                    } else {
                        1
                    }] = 1.0;
                    vector
                })
                .collect())
        }
    }

    #[test]
    fn reranking_filters_weak_evidence_and_cites_ranked_sources() {
        let exact = RagChunkCandidate {
            path: "manual.md".into(),
            text: "Cisco show ip route で経路を確認します".into(),
            brand: "cisco_ios".into(),
            chunk_index: 0,
            distance: 0.2,
        };
        let unrelated = RagChunkCandidate {
            path: "other.md".into(),
            text: "VLAN の基本用語".into(),
            brand: "cisco_ios".into(),
            chunk_index: 0,
            distance: 1.1,
        };
        let result = format_search_results(vec![unrelated, exact], "Cisco show ip route 経路");
        assert_eq!(result.citations.len(), 1);
        assert_eq!(result.citations[0].source_path, "manual.md");
        assert_eq!(result.citations[0].rank, 1);
        assert!(result.output.contains("根拠 [1]"));
        assert!(result.output.contains("類似度スコア:"));
    }

    #[test]
    fn hostname_query_prefers_hostname_chunks_and_caps_chunks_per_source() {
        let make = |path: &str, index: usize, text: &str| RagChunkCandidate {
            path: path.into(),
            text: text.into(),
            brand: String::new(),
            chunk_index: index,
            distance: 0.2,
        };
        let result = format_search_results(
            vec![
                make("vlan.md", 0, "hostname settings are not here"),
                make("vlan.md", 1, "hostname configuration commands"),
                make("vlan.md", 2, "hostname configuration warning"),
                make("other.md", 0, "hostname command syntax"),
            ],
            "hostname",
        );
        assert_eq!(result.citations.len(), 3);
        assert_eq!(
            result
                .citations
                .iter()
                .filter(|citation| citation.source_path == "vlan.md")
                .count(),
            2
        );
        assert!(result
            .citations
            .iter()
            .any(|citation| citation.source_path == "other.md"));
        assert!(result.output.contains("hostname"));
    }

    #[tokio::test]
    async fn graph_vector_search_applies_brand_filter_and_returns_citations() {
        let db_path = std::env::temp_dir().join(format!("mikomai-rag-{}", uuid::Uuid::new_v4()));
        let graph = PortableGraph::initialize_at(&db_path).await.unwrap();
        let rag = PortableRag::new(graph.clone(), Arc::new(TestEmbedder));
        let docs = db_path.join("docs");
        fs::create_dir_all(&docs).unwrap();
        let cisco = docs.join("cisco.md");
        fs::write(&cisco, "---\nbrand: Cisco\ncategory: routing\n---\n# Cisco routes\nCisco show ip route command displays the routing table.").unwrap();
        let juniper = docs.join("juniper.md");
        fs::write(&juniper, "---\nbrand: Juniper\n---\n# Juniper routes\nJuniper show route command displays the route table.").unwrap();

        assert_eq!(rag.ingest_path(&docs).await.unwrap(), 2);
        let result = rag
            .search("Cisco show ip route", Some("cisco"))
            .await
            .unwrap();
        assert_eq!(result.citations.len(), 1);
        assert!(result.citations[0].source_path.ends_with("cisco.md"));
        let previews = rag.previews_for_search_result(&result).await.unwrap();
        assert_eq!(previews[0].title, "Cisco routes");
        assert!(rag
            .expand_selected_documents(&[cisco.to_string_lossy().into_owned()])
            .await
            .unwrap()
            .contains("show ip route"));

        fs::write(
            &cisco,
            "---\nbrand: Cisco\n---\n# Updated manual\nCisco updated only.",
        )
        .unwrap();
        assert_eq!(rag.ingest_path(&cisco).await.unwrap(), 1);
        let refreshed = graph
            .rag_document_chunks(&cisco.to_string_lossy())
            .await
            .unwrap();
        assert_eq!(refreshed.len(), 1);
        assert_eq!(refreshed[0].title, "Updated manual");
        let _ = fs::remove_dir_all(db_path);
    }

    #[test]
    fn embedding_shape_and_brand_alias_contracts_are_checked() {
        assert_eq!(
            normalize_brand("Furukawa Fitelnet").as_deref(),
            Some("furukawa_fitelnet")
        );
        assert!(validate_embedding_batch(&["x".into()], &[vec![0.0; 3]], "test").is_err());
        assert!(validate_embedding_batch(&["x".into()], &[], "test").is_err());
    }
}
