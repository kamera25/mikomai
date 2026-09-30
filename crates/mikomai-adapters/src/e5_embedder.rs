//! Lazy portable multilingual E5-Large embedding adapter used by vector RAG.

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct FastEmbedE5 {
    model: Mutex<Option<TextEmbedding>>,
}

impl Default for FastEmbedE5 {
    fn default() -> Self {
        Self {
            model: Mutex::new(None),
        }
    }
}

impl FastEmbedE5 {
    pub fn new() -> Self {
        Self::default()
    }

    fn cache_path() -> PathBuf {
        if let Some(path) = std::env::var_os("MIKOMAI_E5_CACHE_DIR") {
            return PathBuf::from(path);
        }
        #[cfg(target_os = "macos")]
        {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            home.join("Library/Application Support/MikomaiDesktopMac/models/fastembed")
        }
        #[cfg(not(target_os = "macos"))]
        {
            std::env::var_os("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
                .unwrap_or_else(std::env::temp_dir)
                .join("mikomai/fastembed")
        }
    }
}

impl crate::portable_rag::RagEmbedder for FastEmbedE5 {
    fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, String> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let mut slot = self
            .model
            .lock()
            .map_err(|_| "E5 model lock is poisoned".to_string())?;
        if slot.is_none() {
            let cache = Self::cache_path();
            std::fs::create_dir_all(&cache)
                .map_err(|e| format!("could not create E5 cache {}: {e}", cache.display()))?;
            let options = InitOptions::new(EmbeddingModel::MultilingualE5Large)
                .with_cache_dir(cache)
                .with_show_download_progress(true);
            *slot = Some(
                TextEmbedding::try_new(options)
                    .map_err(|e| format!("could not load multilingual E5-Large: {e}"))?,
            );
        }
        slot.as_ref()
            .expect("model initialized")
            .embed(inputs.to_vec(), Some(16))
            .map(|embeddings| {
                embeddings
                    .into_iter()
                    .map(|embedding| embedding.to_vec())
                    .collect()
            })
            .map_err(|e| format!("multilingual E5 embedding failed: {e}"))
    }
}
