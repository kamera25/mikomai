use crate::state::events::HarnessEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventLog {
    events: Vec<HarnessEvent>,
}

impl EventLog {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn push(&mut self, event: HarnessEvent) {
        self.events.push(event);
    }

    pub fn events(&self) -> &[HarnessEvent] {
        &self.events
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn save_to_path(&self, path: &std::path::Path) -> Result<(), String> {
        let serialized = serde_json::to_vec_pretty(self)
            .map_err(|error| format!("Failed to serialize event log: {error}"))?;
        std::fs::write(path, serialized)
            .map_err(|error| format!("Failed to save event log to {}: {error}", path.display()))
    }

    pub fn load_from_path(path: &std::path::Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|error| {
            format!("Failed to load event log from {}: {error}", path.display())
        })?;
        serde_json::from_slice(&bytes)
            .map_err(|error| format!("Failed to deserialize event log: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::events::HarnessEvent;

    #[test]
    fn persists_and_restores_task_events() {
        let path =
            std::env::temp_dir().join(format!("mikomai-event-log-{}.json", uuid::Uuid::new_v4()));
        let task_id = uuid::Uuid::new_v4();
        let mut log = EventLog::new();
        log.push(HarnessEvent::TaskStarted {
            task_id,
            timestamp: chrono::Utc::now(),
        });
        log.save_to_path(&path).unwrap();
        let restored = EventLog::load_from_path(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert!(
            matches!(restored.events().first(), Some(HarnessEvent::TaskStarted { task_id: restored_id, .. }) if *restored_id == task_id)
        );
    }
}
