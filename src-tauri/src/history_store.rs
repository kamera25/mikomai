//! SurrealDB persistence for chat history.
//!
//! This module owns the chat-history schema and queries. `graph.rs` remains
//! limited to network inventory data even though both domains share one local
//! embedded SurrealDB instance.

use chrono::Utc;
use serde_json::{json, Value};

use crate::graph::SurrealDbState;

pub async fn initialize(db: &SurrealDbState) -> Result<(), String> {
    db.db
        .query("DEFINE TABLE chat_history SCHEMALESS;")
        .await
        .map_err(|e| format!("Failed to define chat history schema: {e}"))?;
    Ok(())
}

pub async fn load(db: &SurrealDbState) -> Result<Option<Value>, String> {
    let mut response = db
        .db
        .query("SELECT history FROM chat_history:primary;")
        .await
        .map_err(|e| format!("Failed to read chat history: {e}"))?;
    let records: Vec<Value> = response
        .take(0)
        .map_err(|e| format!("Failed to decode chat history: {e}"))?;
    Ok(records
        .into_iter()
        .next()
        .and_then(|record| record.get("history").cloned()))
}

pub async fn save(db: &SurrealDbState, history: Value) -> Result<(), String> {
    db.db
        .query("UPSERT chat_history:primary CONTENT $record;")
        .bind((
            "record",
            json!({
                "history": history,
                "updated_at": Utc::now(),
            }),
        ))
        .await
        .map_err(|e| format!("Failed to write chat history: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn persists_chat_history_in_embedded_surrealdb() {
        let path = std::env::temp_dir().join(format!(
            "mikomai-chat-history-test-{}",
            uuid::Uuid::new_v4()
        ));
        let state = SurrealDbState::initialize_at(&path).await.unwrap();
        initialize(&state).await.unwrap();
        let history = json!([{ "id": "session-1", "type": "session", "messages": [] }]);

        save(&state, history.clone()).await.unwrap();
        assert_eq!(load(&state).await.unwrap(), Some(history));

        drop(state);
        let _ = std::fs::remove_dir_all(path);
    }
}
