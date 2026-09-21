//! Search adapter boundary. A real Surreal/fastembed implementation can be
//! swapped in without changing the core SearchPort contract.
use mikomai_core::port::{PortFuture, SearchHit, SearchPort};

#[derive(Default, Clone)]
pub struct StaticSearch {
    pub documents: Vec<SearchHit>,
}
impl SearchPort for StaticSearch {
    fn search<'a>(&'a self, query: &'a str, limit: usize) -> PortFuture<'a, Vec<SearchHit>> {
        Box::pin(async move {
            let query = query.to_lowercase();
            Ok(self
                .documents
                .iter()
                .filter(|hit| {
                    hit.title.to_lowercase().contains(&query)
                        || hit.content.to_lowercase().contains(&query)
                })
                .take(limit)
                .cloned()
                .collect())
        })
    }
}
