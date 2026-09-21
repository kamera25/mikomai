//! Markdown knowledge-store adapter used by the standalone CLI.
use mikomai_core::port::{PlanDecision, PlannerPort, PortFuture, SearchHit, SearchPort};
use mikomai_core::TaskSnapshot;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeDocument {
    pub source: String,
    pub title: String,
    pub content: String,
}
pub struct KnowledgeStore {
    root: PathBuf,
}
impl KnowledgeStore {
    pub fn at(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }
    pub fn ingest(&self, input: impl AsRef<Path>) -> Result<usize, String> {
        let mut files = Vec::new();
        collect_markdown(input.as_ref(), &mut files)?;
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        for file in &files {
            let raw = fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
            let doc = KnowledgeDocument {
                source: file.to_string_lossy().into_owned(),
                title: title(&raw, file),
                content: raw,
            };
            let key = format!("doc-{:016x}", stable_hash(&doc.source));
            let path = self.root.join(format!("{key}.json"));
            fs::write(path, serde_json::to_vec(&doc).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        }
        Ok(files.len())
    }
    fn search_sync(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, String> {
        let terms = query_terms(query);
        if terms.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let mut hits: Vec<(usize, SearchHit)> = Vec::new();
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        for entry in fs::read_dir(&self.root).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(bytes) = fs::read(&path) else { continue };
            let Ok(doc) = serde_json::from_slice::<KnowledgeDocument>(&bytes) else {
                continue;
            };
            let haystack = format!("{}\n{}", doc.title, doc.content).to_lowercase();
            let matched = terms
                .iter()
                .filter(|term| haystack.contains(term.as_str()))
                .count();
            if matched > 0 {
                hits.push((
                    matched,
                    SearchHit {
                        title: doc.title,
                        content: doc.content,
                        score: Some(matched as f32 / terms.len() as f32),
                    },
                ));
            }
        }
        hits.sort_by(|left, right| right.0.cmp(&left.0));
        Ok(hits.into_iter().take(limit).map(|(_, hit)| hit).collect())
    }
}
impl SearchPort for KnowledgeStore {
    fn search<'a>(&'a self, query: &'a str, limit: usize) -> PortFuture<'a, Vec<SearchHit>> {
        Box::pin(async move { self.search_sync(query, limit) })
    }
}

/// Worker planner backed by the local Markdown knowledge base. It deliberately
/// produces a factual completion brief; presentation remains the core
/// ChatService/reporter responsibility.
pub struct KnowledgePlanner<'a> {
    store: &'a KnowledgeStore,
    limit: usize,
}
impl<'a> KnowledgePlanner<'a> {
    pub fn new(store: &'a KnowledgeStore) -> Self {
        Self { store, limit: 3 }
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit.max(1);
        self
    }
}
impl<'a> PlannerPort for KnowledgePlanner<'a> {
    fn plan<'b>(&'b self, task: &'b TaskSnapshot) -> PortFuture<'b, PlanDecision> {
        Box::pin(async move {
            let hits = self.store.search(&task.task.goal, self.limit).await?;
            if hits.is_empty() {
                return Ok(PlanDecision::Complete {
                    brief: format!(
                        "ローカルナレッジベースに該当する資料がありませんでした。質問: {}",
                        task.task.goal
                    ),
                });
            }
            Ok(PlanDecision::Complete {
                brief: format_answer(&hits),
            })
        })
    }
}

fn format_answer(hits: &[SearchHit]) -> String {
    hits.iter()
        .enumerate()
        .map(|(index, hit)| {
            format!(
                "### 根拠 {}: {}\n{}",
                index + 1,
                hit.title,
                hit.content.trim()
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
fn collect_markdown(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_file() {
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("md"))
        {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }
    for entry in fs::read_dir(path).map_err(|e| format!("{}: {e}", path.display()))? {
        collect_markdown(&entry.map_err(|e| e.to_string())?.path(), files)?;
    }
    Ok(())
}
fn title(raw: &str, path: &Path) -> String {
    raw.lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix('#')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        })
        .or_else(|| path.file_stem().and_then(|s| s.to_str()).map(str::to_owned))
        .unwrap_or_else(|| "Untitled".into())
}
fn stable_hash(value: &str) -> u64 {
    value.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

fn query_terms(query: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut current = String::new();
    let mut current_is_ascii = false;
    for character in query.chars() {
        if character.is_ascii_alphanumeric() {
            if !current.is_empty() && !current_is_ascii {
                if current.chars().count() >= 2 {
                    terms.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
            }
            current_is_ascii = true;
            current.push(character.to_ascii_lowercase());
        } else if ('\u{3040}'..='\u{30ff}').contains(&character)
            || ('\u{4e00}'..='\u{9fff}').contains(&character)
        {
            if !current.is_empty() && current_is_ascii {
                terms.push(std::mem::take(&mut current));
            }
            current_is_ascii = false;
            current.push(character);
        } else if current.chars().count() >= 2 {
            terms.push(std::mem::take(&mut current));
            current_is_ascii = false;
        } else {
            current.clear();
            current_is_ascii = false;
        }
    }
    if current.chars().count() >= 2 {
        terms.push(current);
    }
    terms.sort();
    terms.dedup();
    terms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searches_mixed_japanese_and_ascii_terms() {
        let root = std::env::temp_dir().join(format!("mikomai-knowledge-{}", uuid::Uuid::new_v4()));
        let input = root.join("input");
        let index = root.join("index");
        std::fs::create_dir_all(&input).unwrap();
        std::fs::write(
            input.join("f220.md"),
            "# F220 VLAN\nFitelnet の VLAN 設定方法。\n",
        )
        .unwrap();
        let store = KnowledgeStore::at(&index);
        assert_eq!(store.ingest(&input).unwrap(), 1);
        let hits =
            futures_lite::future::block_on(store.search("F220のVLAN設定方法を教えて", 3)).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].content.contains("VLAN"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn planner_returns_a_cited_completion_brief() {
        let root = std::env::temp_dir().join(format!("mikomai-planner-{}", uuid::Uuid::new_v4()));
        let input = root.join("input");
        let index = root.join("index");
        std::fs::create_dir_all(&input).unwrap();
        std::fs::write(input.join("vlan.md"), "# VLAN\nuse vlan-id 10\n").unwrap();
        let store = KnowledgeStore::at(&index);
        store.ingest(&input).unwrap();
        let planner = KnowledgePlanner::new(&store);
        let decision =
            futures_lite::future::block_on(planner.plan(&TaskSnapshot::new("VLAN"))).unwrap();
        assert!(
            matches!(decision, PlanDecision::Complete { brief } if brief.contains("根拠 1") && brief.contains("vlan-id 10"))
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
