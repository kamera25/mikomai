use arrow_array::RecordBatch;
use lancedb::Table;

pub trait RagSearcher {
    async fn search(
        &self,
        table: &Table,
        query: &str,
        filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<RecordBatch>, String>;
}
