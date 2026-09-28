use crate::application::DesktopTaskManager;
use crate::commands::chat::TaskDto;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub fn start_task(goal: String, state: State<'_, DesktopTaskManager>) -> Result<TaskDto, String> {
    state
        .start(goal)
        .map(TaskDto::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn resume_task(
    task_id: Uuid,
    state: State<'_, DesktopTaskManager>,
) -> Result<Option<TaskDto>, String> {
    state
        .resume(task_id)
        .map(|snapshot| snapshot.map(TaskDto::from))
        .map_err(|error| error.to_string())
}

pub use crate::node_refresh::start_node_db_bulk_refresh;
pub use crate::task_audit::{get_agent_task_audit, list_agent_tasks, resume_agent_task};
