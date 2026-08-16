use arrow_array::RecordBatch;
use fastembed::TextEmbedding;
use futures::StreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::Table;
use std::sync::Arc;

use super::traits::RagSearcher;

pub struct VectorSearcher
{
    model: Arc<TextEmbedding>,
    task_description: String,
}

impl VectorSearcher
{
    pub fn new(model: Arc<TextEmbedding>, task_description: impl Into<String>) -> Self
    {
        Self {
            model,
            task_description: task_description.into(),
        }
    }
}

impl RagSearcher for VectorSearcher
{
    async fn search(
        &self,
        table: &Table,
        query: &str,
        filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<RecordBatch>, String>
    {
        log::info!("Executing Vector Search fallback for query: {}", query);

        let instructional_query = format!("Instruct: {}\nQuery: {}", self.task_description, query);
        let embeddings = self
            .model
            .embed(vec![instructional_query], None)
            .map_err(|e| format!("Embedding error: {}", e))?;
        let query_vector = embeddings
            .first()
            .ok_or("Failed to generate embedding")?
            .clone();

        let mut batches = Vec::new();

        if let Some(filter_str) = filter
        {
            log::info!(
                "Executing LanceDB Pre-filtering with condition: {}",
                filter_str
            );
            let pre_query = table
                .query()
                .nearest_to(query_vector.clone())
                .map_err(|e| format!("Vector search error: {}", e))?
                .only_if(filter_str.to_string())
                .limit(limit);

            if let Ok(mut stream) = pre_query.execute().await
            {
                while let Some(Ok(batch)) = stream.next().await
                {
                    if batch.num_rows() > 0
                    {
                        batches.push(batch);
                    }
                }
            }

            // 事前フィルタで結果が得られなかった場合、事後フィルタ（Post-filtering）へフォールバック
            if batches.is_empty()
            {
                log::info!(
                    "Pre-filtering returned no results. Falling back to Post-filtering: {}",
                    filter_str
                );
                let post_query = table
                    .query()
                    .nearest_to(query_vector.clone())
                    .map_err(|e| format!("Vector search error: {}", e))?
                    .only_if(filter_str.to_string())
                    .postfilter()
                    .limit(limit);

                if let Ok(mut stream) = post_query.execute().await
                {
                    while let Some(Ok(batch)) = stream.next().await
                    {
                        if batch.num_rows() > 0
                        {
                            batches.push(batch);
                        }
                    }
                }
            }
        }
        else
        {
            // フィルタなし通常ベクトル検索
            let query = table
                .query()
                .nearest_to(query_vector)
                .map_err(|e| format!("Vector search error: {}", e))?
                .limit(limit);

            if let Ok(mut stream) = query.execute().await
            {
                while let Some(Ok(batch)) = stream.next().await
                {
                    if batch.num_rows() > 0
                    {
                        batches.push(batch);
                    }
                }
            }
        }

        Ok(batches)
    }
}
