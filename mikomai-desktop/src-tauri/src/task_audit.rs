//! Read-only access to persisted agent event logs and safe task continuation.

use crate::harness::agent_loop::AgentLoop;
use crate::state::event_log::EventLog;
use crate::state::events::HarnessEvent;
use crate::state::network_state::NetworkState;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State, Window};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTaskSummary {
    pub task_id: uuid::Uuid,
    pub started_at: DateTime<Utc>,
    pub goal: String,
    pub last_event_at: DateTime<Utc>,
    pub event_count: usize,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTaskAudit {
    pub summary: AgentTaskSummary,
    pub events: Vec<HarnessEvent>,
}

fn event_directory(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Failed to resolve agent event storage: {error}"))?
        .join("agent-events"))
}

fn event_time(event: &HarnessEvent) -> DateTime<Utc> {
    match event {
        HarnessEvent::TaskStarted { timestamp, .. }
        | HarnessEvent::GoalSet { timestamp, .. }
        | HarnessEvent::StateUpdated { timestamp, .. }
        | HarnessEvent::Finished { timestamp, .. } => *timestamp,
        HarnessEvent::Observation(observation) => observation.timestamp,
        HarnessEvent::Decision(decision) => decision.timestamp,
        HarnessEvent::Action(action) => action.timestamp,
        HarnessEvent::Result(result) => result.timestamp,
    }
}

fn summary_from_log(task_id: uuid::Uuid, log: &EventLog) -> Result<AgentTaskSummary, String> {
    let started_at = log
        .events()
        .iter()
        .find_map(|event| match event {
            HarnessEvent::TaskStarted { timestamp, .. } => Some(*timestamp),
            _ => None,
        })
        .ok_or("Agent event log has no task start event".to_string())?;
    let goal = log
        .events()
        .iter()
        .rev()
        .find_map(|event| match event {
            HarnessEvent::GoalSet { goal, .. } => Some(goal.clone()),
            _ => None,
        })
        .ok_or("Agent event log has no goal".to_string())?;
    let last_event_at = log.events().last().map(event_time).unwrap_or(started_at);
    let status = match log.events().last() {
        Some(HarnessEvent::Finished { .. }) => "finished",
        _ => "stopped",
    }
    .to_string();
    Ok(AgentTaskSummary {
        task_id,
        started_at,
        goal,
        last_event_at,
        event_count: log.len(),
        status,
    })
}

fn audit_from_log(task_id: uuid::Uuid, log: &EventLog) -> Result<AgentTaskAudit, String> {
    Ok(AgentTaskAudit {
        summary: summary_from_log(task_id, log)?,
        events: log.events().to_vec(),
    })
}

fn summaries_from_directory(directory: &Path) -> Result<Vec<AgentTaskSummary>, String> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("Failed to read agent event storage: {error}")),
    };
    let mut tasks = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let Ok(task_id) = uuid::Uuid::parse_str(stem) else {
            continue;
        };
        match EventLog::load_from_path(&path).and_then(|log| summary_from_log(task_id, &log)) {
            Ok(summary) => tasks.push(summary),
            Err(error) => log::warn!(
                "Skipping unreadable agent event log {}: {error}",
                path.display()
            ),
        }
    }
    tasks.sort_by(|left, right| right.last_event_at.cmp(&left.last_event_at));
    Ok(tasks)
}

fn load_log(app: &AppHandle, task_id: uuid::Uuid) -> Result<EventLog, String> {
    EventLog::load_from_path(&event_directory(app)?.join(format!("{task_id}.json")))
}

#[tauri::command]
pub fn list_agent_tasks(app: AppHandle) -> Result<Vec<AgentTaskSummary>, String> {
    summaries_from_directory(&event_directory(&app)?)
}

#[tauri::command]
pub fn get_agent_task_audit(app: AppHandle, task_id: uuid::Uuid) -> Result<AgentTaskAudit, String> {
    let log = load_log(&app, task_id)?;
    audit_from_log(task_id, &log)
}

/// Continues an investigation from its recorded observations. This deliberately
/// starts a new task record, preserving the original audit trail unchanged.
#[tauri::command]
pub async fn resume_agent_task(
    app: AppHandle,
    window: Window,
    llama_state: State<'_, crate::llm::llm::LlamaState>,
    task_id: uuid::Uuid,
) -> Result<String, String> {
    crate::llm::loader::ensure_model_loaded(&app, &llama_state).await?;
    let log = load_log(&app, task_id)?;
    let state = NetworkState::rebuild_from_log(&log);
    let goal = state
        .desired
        .as_ref()
        .map(|desired| desired.raw_goal.clone())
        .ok_or("The selected task has no resumable goal".to_string())?;
    let mut agent = AgentLoop::new(app, window, 10);
    agent.network_state = state;
    agent.run(goal, &llama_state).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn write_log(directory: &Path, task_id: uuid::Uuid, log: &EventLog) {
        log.save_to_path(&directory.join(format!("{task_id}.json")))
            .unwrap();
    }

    fn valid_log(
        task_id: uuid::Uuid,
        started_at: DateTime<Utc>,
        last_at: DateTime<Utc>,
    ) -> EventLog {
        let mut log = EventLog::new();
        log.push(HarnessEvent::TaskStarted {
            task_id,
            timestamp: started_at,
        });
        log.push(HarnessEvent::GoalSet {
            goal: format!("goal {task_id}"),
            timestamp: started_at,
        });
        log.push(HarnessEvent::Finished {
            reason: "done".into(),
            timestamp: last_at,
        });
        log
    }

    #[test]
    fn summarizes_a_completed_log() {
        let task_id = uuid::Uuid::new_v4();
        let time = Utc::now();
        let mut log = EventLog::new();
        log.push(HarnessEvent::TaskStarted {
            task_id,
            timestamp: time,
        });
        log.push(HarnessEvent::GoalSet {
            goal: "R1 を調査".into(),
            timestamp: time,
        });
        log.push(HarnessEvent::Finished {
            reason: "done".into(),
            timestamp: time,
        });
        let summary = summary_from_log(task_id, &log).unwrap();
        assert_eq!(summary.goal, "R1 を調査");
        assert_eq!(summary.status, "finished");
        assert_eq!(summary.event_count, 3);
    }

    #[test]
    fn rejects_empty_logs_and_logs_missing_start_or_goal() {
        let task_id = uuid::Uuid::new_v4();
        assert!(summary_from_log(task_id, &EventLog::new())
            .unwrap_err()
            .contains("no task start event"));

        let time = Utc::now();
        let mut missing_start = EventLog::new();
        missing_start.push(HarnessEvent::GoalSet {
            goal: "goal".into(),
            timestamp: time,
        });
        assert!(summary_from_log(task_id, &missing_start)
            .unwrap_err()
            .contains("no task start event"));

        let mut missing_goal = EventLog::new();
        missing_goal.push(HarnessEvent::TaskStarted {
            task_id,
            timestamp: time,
        });
        assert!(summary_from_log(task_id, &missing_goal)
            .unwrap_err()
            .contains("no goal"));
    }

    #[test]
    fn list_skips_corrupt_and_invalid_uuid_entries_and_sorts_by_last_event() {
        let directory =
            std::env::temp_dir().join(format!("mikomai-task-audit-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let older_id = uuid::Uuid::new_v4();
        let newer_id = uuid::Uuid::new_v4();
        let now = Utc::now();
        write_log(
            &directory,
            older_id,
            &valid_log(older_id, now - chrono::Duration::minutes(4), now),
        );
        write_log(
            &directory,
            newer_id,
            &valid_log(
                newer_id,
                now - chrono::Duration::minutes(2),
                now + chrono::Duration::minutes(1),
            ),
        );
        fs::write(directory.join("not-a-uuid.json"), "{}").unwrap();
        fs::write(
            directory.join(format!("{}.json", uuid::Uuid::new_v4())),
            "not-json",
        )
        .unwrap();

        let summaries = summaries_from_directory(&directory).unwrap();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].task_id, newer_id);
        assert_eq!(summaries[1].task_id, older_id);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn detail_summary_matches_its_event_log() {
        let task_id = uuid::Uuid::new_v4();
        let now = Utc::now();
        let log = valid_log(task_id, now, now + chrono::Duration::seconds(3));
        let audit = audit_from_log(task_id, &log).unwrap();

        assert_eq!(audit.summary.task_id, task_id);
        assert_eq!(audit.summary.event_count, audit.events.len());
        assert_eq!(
            audit.summary.last_event_at,
            event_time(audit.events.last().unwrap())
        );
        assert!(matches!(
            audit.events.last(),
            Some(HarnessEvent::Finished { .. })
        ));
    }
}
