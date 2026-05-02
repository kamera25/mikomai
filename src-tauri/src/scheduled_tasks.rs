use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State, Emitter};
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};
use uuid::Uuid;
use chrono::Local;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTask {
    pub id: String,
    pub name: String,
    pub status: String,
    pub schedule: String,
    pub last_run: String,
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
                    println!("Executing scheduled task: {}", task_id);

                    // Actually execute logic here (mocked for now, or just emit event and update last run)
                    let _ = app_handle.emit("task-executed", task_id.clone());

                    let state: State<SchedulerState> = app_handle.state();
                    let mut tasks = state.tasks.lock().await;
                    if let Some(t) = tasks.iter_mut().find(|t| t.id == task_id) {
                        t.last_run = Local::now().format("%Y-%m-%d %H:%M").to_string();
                    }

                    let path = get_tasks_path(&app_handle);
                    if let Ok(data) = serde_json::to_string_pretty(&*tasks) {
                        let _ = fs::write(path, data);
                    }
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
                    println!("Executing scheduled task: {}", task_id);
                    let _ = app_handle.emit("task-executed", task_id.clone());

                    let state: State<SchedulerState> = app_handle.state();
                    let mut tasks = state.tasks.lock().await;
                    if let Some(t) = tasks.iter_mut().find(|t| t.id == task_id) {
                        t.last_run = Local::now().format("%Y-%m-%d %H:%M").to_string();
                    }

                    let path = get_tasks_path(&app_handle);
                    if let Ok(data) = serde_json::to_string_pretty(&*tasks) {
                        let _ = fs::write(path, data);
                    }
                })
            });

            if let Ok(j) = job {
                let _ = new_sched.add(j).await;
            } else {
                println!("Failed to parse cron expression: {}", cron_expr);
            }
        }
    }

    new_sched.start().await.expect("Failed to start new scheduler");

    // Replace the active scheduler
    let mut sched_lock = sched_state.sched.lock().await;
    *sched_lock = new_sched;
}

#[tauri::command]
pub async fn load_scheduled_tasks(state: tauri::State<'_, SchedulerState>) -> Result<Vec<ScheduledTask>, String> {
    let tasks = state.tasks.lock().await;
    Ok(tasks.clone())
}

#[tauri::command]
pub async fn save_scheduled_tasks(app: tauri::AppHandle, tasks: Vec<ScheduledTask>, state: tauri::State<'_, SchedulerState>) -> Result<(), String> {
    {
        let mut state_tasks = state.tasks.lock().await;
        *state_tasks = tasks.clone();
    }

    let path = get_tasks_path(&app);
    let data = serde_json::to_string_pretty(&tasks).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())?;

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
) -> Result<ScheduledTask, String> {
    let task = ScheduledTask {
        id: Uuid::new_v4().to_string(),
        name,
        status: "running".to_string(),
        schedule,
        last_run: "-".to_string(),
        prompt,
    };

    {
        let mut tasks = state.tasks.lock().await;
        tasks.push(task.clone());
        let path = get_tasks_path(&app);
        let data = serde_json::to_string_pretty(&*tasks).map_err(|e| e.to_string())?;
        let _ = fs::write(path, data);
    }

    restart_scheduler(&app, &*state).await;

    Ok(task)
}

#[tauri::command]
pub async fn update_scheduled_task(
    app: tauri::AppHandle,
    task: ScheduledTask,
    state: tauri::State<'_, SchedulerState>
) -> Result<(), String> {
    {
        let mut tasks = state.tasks.lock().await;
        if let Some(pos) = tasks.iter().position(|t| t.id == task.id) {
            tasks[pos] = task;
        }
        let path = get_tasks_path(&app);
        let data = serde_json::to_string_pretty(&*tasks).map_err(|e| e.to_string())?;
        let _ = fs::write(path, data);
    }

    restart_scheduler(&app, &*state).await;

    Ok(())
}

#[tauri::command]
pub async fn delete_scheduled_task(
    app: tauri::AppHandle,
    id: String,
    state: tauri::State<'_, SchedulerState>
) -> Result<(), String> {
    {
        let mut tasks = state.tasks.lock().await;
        tasks.retain(|t| t.id != id);
        let path = get_tasks_path(&app);
        let data = serde_json::to_string_pretty(&*tasks).map_err(|e| e.to_string())?;
        let _ = fs::write(path, data);
    }

    restart_scheduler(&app, &*state).await;

    Ok(())
}

#[tauri::command]
pub async fn execute_task(
    app: tauri::AppHandle,
    id: String,
    state: tauri::State<'_, SchedulerState>
) -> Result<(), String> {
    println!("Manually executing task {}", id);
    let mut tasks = state.tasks.lock().await;
    if let Some(t) = tasks.iter_mut().find(|task| task.id == id) {
        t.last_run = Local::now().format("%Y-%m-%d %H:%M").to_string();

        let path = get_tasks_path(&app);
        if let Ok(data) = serde_json::to_string_pretty(&*tasks) {
            let _ = fs::write(path, data);
        }
    }
    // Logic to actually execute would go here
    Ok(())
}
