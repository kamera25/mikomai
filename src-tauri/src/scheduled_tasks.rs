use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State, Emitter};
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};
use uuid::Uuid;
use chrono::Local;
use tracing::Instrument;
use crate::error::TauriError;
use validator::{Validate, ValidationError};

pub fn validate_cron_expression(schedule: &str) -> Result<(), ValidationError> {
    if Job::new_async(schedule, |_uuid, _l| Box::pin(async move {})).is_ok() {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_cron_expression"))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ScheduledTaskError {
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization/Deserialization error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Serialize, Deserialize, Clone, Debug, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTask {
    pub id: String,
    #[validate(length(min = 1))]
    pub name: String,
    pub status: String,
    #[validate(custom(function = "validate_cron_expression"))]
    pub schedule: String,
    pub last_run: String,
    #[validate(length(min = 1))]
    pub prompt: String,
}

pub struct SchedulerState {
    pub sched: Arc<Mutex<JobScheduler>>,
    pub tasks: Arc<Mutex<Vec<ScheduledTask>>>,
}

fn get_tasks_path(app: &tauri::AppHandle) -> PathBuf {
    let path = app.path().app_data_dir().expect("Failed to get app data dir");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path.join("scheduled_tasks.json")
}

pub async fn init_scheduler(app: &AppHandle) -> SchedulerState {
    let tasks_path = get_tasks_path(app);
    let mut tasks = Vec::new();
    if tasks_path.exists() {
        if let Ok(data) = fs::read_to_string(&tasks_path) {
            if let Ok(loaded_tasks) = serde_json::from_str::<Vec<ScheduledTask>>(&data) {
                tasks = loaded_tasks;
            }
        }
    }

    let sched = JobScheduler::new().await.expect("Failed to create JobScheduler");

    let tasks_state = Arc::new(Mutex::new(tasks.clone()));

    // Add jobs for existing tasks
    for task in tasks {
        if task.status == "running" {
            let task_id = task.id.clone();
            let app_handle = app.clone();
            let cron_expr = task.schedule.clone();

            let job = Job::new_async(cron_expr.as_str(), move |_uuid, mut _l| {
                let task_id = task_id.clone();
                let app_handle = app_handle.clone();
                Box::pin(async move {
                    let task_id_clone = task_id.clone();
                    let app_handle_clone = app_handle.clone();
                    async move {
                        tracing::info!("Executing scheduled task");

                        // Actually execute logic here (mocked for now, or just emit event and update last run)
                        let _ = app_handle_clone.emit("task-executed", task_id_clone.clone());

                        let state: State<SchedulerState> = app_handle_clone.state();
                        let mut tasks = state.tasks.lock().await;
                        if let Some(t) = tasks.iter_mut().find(|t| t.id == task_id_clone) {
                            t.last_run = Local::now().format("%Y-%m-%d %H:%M").to_string();
                        }

                        let path = get_tasks_path(&app_handle_clone);
                        if let Ok(data) = serde_json::to_string_pretty(&*tasks) {
                            let _ = fs::write(path, data);
                        }
                    }
                    .instrument(tracing::info_span!("execute_scheduled_task", task_id = %task_id))
                    .await;
                })
            });

            if let Ok(j) = job {
                let _ = sched.add(j).await;
            }
        }
    }

    sched.start().await.expect("Failed to start scheduler");

    SchedulerState {
        sched: Arc::new(Mutex::new(sched)),
        tasks: tasks_state,
    }
}

pub async fn restart_scheduler(app: &AppHandle, sched_state: &SchedulerState) {
    let tasks = sched_state.tasks.lock().await.clone();

    // Create new scheduler to replace the old one
    let new_sched = JobScheduler::new().await.expect("Failed to create new JobScheduler");

    for task in tasks {
        if task.status == "running" {
            let task_id = task.id.clone();
            let app_handle = app.clone();
            let cron_expr = task.schedule.clone();

            let job = Job::new_async(cron_expr.as_str(), move |_uuid, mut _l| {
                let task_id = task_id.clone();
                let app_handle = app_handle.clone();
                Box::pin(async move {
                    let task_id_clone = task_id.clone();
                    let app_handle_clone = app_handle.clone();
                    async move {
                        tracing::info!("Executing scheduled task");
                        let _ = app_handle_clone.emit("task-executed", task_id_clone.clone());

                        let state: State<SchedulerState> = app_handle_clone.state();
                        let mut tasks = state.tasks.lock().await;
                        if let Some(t) = tasks.iter_mut().find(|t| t.id == task_id_clone) {
                            t.last_run = Local::now().format("%Y-%m-%d %H:%M").to_string();
                        }

                        let path = get_tasks_path(&app_handle_clone);
                        if let Ok(data) = serde_json::to_string_pretty(&*tasks) {
                            let _ = fs::write(path, data);
                        }
                    }
                    .instrument(tracing::info_span!("execute_scheduled_task", task_id = %task_id))
                    .await;
                })
            });

            if let Ok(j) = job {
                let _ = new_sched.add(j).await;
            } else {
                tracing::error!(cron_expr = %cron_expr, "Failed to parse cron expression");
            }
        }
    }

    new_sched.start().await.expect("Failed to start new scheduler");

    // Replace the active scheduler
    let mut sched_lock = sched_state.sched.lock().await;
    *sched_lock = new_sched;
}

#[tauri::command]
pub async fn load_scheduled_tasks(state: tauri::State<'_, SchedulerState>) -> Result<Vec<ScheduledTask>, TauriError> {
    let tasks = state.tasks.lock().await;
    Ok(tasks.clone())
}

#[tauri::command]
pub async fn save_scheduled_tasks(app: tauri::AppHandle, tasks: Vec<ScheduledTask>, state: tauri::State<'_, SchedulerState>) -> Result<(), TauriError> {
    for task in &tasks {
        task.validate().map_err(|e| TauriError(crate::error::MikomaiError::Validation(e.to_string())))?;
    }
    {
        let mut state_tasks = state.tasks.lock().await;
        *state_tasks = tasks.clone();
    }

    let path = get_tasks_path(&app);
    let data = serde_json::to_string_pretty(&tasks)?;
    fs::write(path, data)?;

    restart_scheduler(&app, &*state).await;

    Ok(())
}

#[tauri::command]
pub async fn add_scheduled_task(
    app: tauri::AppHandle,
    name: String,
    schedule: String,
    prompt: String,
    state: tauri::State<'_, SchedulerState>
) -> Result<ScheduledTask, TauriError> {
    let task = ScheduledTask {
        id: Uuid::new_v4().to_string(),
        name,
        status: "running".to_string(),
        schedule,
        last_run: "-".to_string(),
        prompt,
    };

    task.validate().map_err(|e| TauriError(crate::error::MikomaiError::Validation(e.to_string())))?;

    let path = get_tasks_path(&app);
    {
        let mut tasks = state.tasks.lock().await;
        tasks.push(task.clone());
        let data = serde_json::to_string_pretty(&*tasks)?;
        fs::write(path, data)?;
    }

    restart_scheduler(&app, &*state).await;

    Ok(task)
}

#[tauri::command]
pub async fn update_scheduled_task(
    app: tauri::AppHandle,
    task: ScheduledTask,
    state: tauri::State<'_, SchedulerState>
) -> Result<(), TauriError> {
    task.validate().map_err(|e| TauriError(crate::error::MikomaiError::Validation(e.to_string())))?;

    let path = get_tasks_path(&app);
    {
        let mut tasks = state.tasks.lock().await;
        if let Some(pos) = tasks.iter().position(|t| t.id == task.id) {
            tasks[pos] = task;
        }
        let data = serde_json::to_string_pretty(&*tasks)?;
        fs::write(path, data)?;
    }

    restart_scheduler(&app, &*state).await;

    Ok(())
}

#[tauri::command]
pub async fn delete_scheduled_task(
    app: tauri::AppHandle,
    id: String,
    state: tauri::State<'_, SchedulerState>
) -> Result<(), TauriError> {
    let path = get_tasks_path(&app);
    {
        let mut tasks = state.tasks.lock().await;
        tasks.retain(|t| t.id != id);
        let data = serde_json::to_string_pretty(&*tasks)?;
        fs::write(path, data)?;
    }

    restart_scheduler(&app, &*state).await;

    Ok(())
}

#[tauri::command]
pub async fn execute_task(
    app: tauri::AppHandle,
    id: String,
    state: tauri::State<'_, SchedulerState>
) -> Result<(), TauriError> {
    let span = tracing::info_span!("execute_task", task_id = %id);
    async move {
        tracing::info!("Manually executing task");
        let mut tasks = state.tasks.lock().await;
        if let Some(t) = tasks.iter_mut().find(|task| task.id == id) {
            t.last_run = Local::now().format("%Y-%m-%d %H:%M").to_string();

            let path = get_tasks_path(&app);
            let data = serde_json::to_string_pretty(&*tasks)?;
            fs::write(path, data)?;
        }
        // Logic to actually execute would go here
        Ok(())
    }
    .instrument(span)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduled_task_serialization() {
        let task = ScheduledTask {
            id: "task-1".to_string(),
            name: "Test Task".to_string(),
            status: "running".to_string(),
            schedule: "0 0 * * * *".to_string(),
            last_run: "2023-10-27 10:00".to_string(),
            prompt: "Test prompt".to_string(),
        };

        let serialized = serde_json::to_string(&task).unwrap();
        assert!(serialized.contains(r#""id":"task-1""#));
        assert!(serialized.contains(r#""name":"Test Task""#));
        assert!(serialized.contains(r#""schedule":"0 0 * * * *""#));
    }
}
