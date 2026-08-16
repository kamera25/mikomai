use arrow_array::RecordBatch;
use futures::StreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::Table;

use super::traits::RagSearcher;

pub struct FullTextSearcher;

impl FullTextSearcher
{
    pub fn new() -> Self
    {
        Self
    }
}

impl Default for FullTextSearcher
{
    fn default() -> Self
    {
        Self::new()
    }
}

impl RagSearcher for FullTextSearcher
{
    async fn search(
        &self,
        table: &Table,
        query: &str,
        filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<RecordBatch>, String>
    {
        if query.is_empty()
        {
            return Ok(Vec::new());
        }

        log::info!(
            "Executing LanceDB Full-Text Search (FTS) [Primary] for query: {}",
            query
        );
        let fts_query = lancedb::index::scalar::FullTextSearchQuery::new(query.to_string());
        let mut fts_builder = table.query().full_text_search(fts_query).limit(limit);
        if let Some(filter_str) = filter
        {
            fts_builder = fts_builder.only_if(filter_str.to_string());
        }

        let mut batches = Vec::new();
        if let Ok(mut stream) = fts_builder.execute().await
        {
            while let Some(Ok(batch)) = stream.next().await
            {
                if batch.num_rows() > 0
                {
                    batches.push(batch);
                }
            }
        }

        Ok(batches)
    }
}
