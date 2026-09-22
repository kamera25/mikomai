//! Desktop composition root. Domain behavior is supplied by mikomai-core;
//! this module is intentionally limited to wiring Tauri-owned services.
use mikomai_adapters::memory::InMemoryTaskRepository;
use mikomai_adapters::persistence::FileTaskRepository;
use mikomai_core::{TaskManager, TaskRepository};
use std::path::Path;
use tauri::Manager;

pub type DesktopApplication<R> = TaskManager<R>;
pub type DesktopTaskManager = TaskManager<FileTaskRepository>;

pub fn compose<R: TaskRepository>(repository: R) -> DesktopApplication<R> {
    TaskManager::new(repository)
}

/// Default desktop composition used by development commands and tests.
pub fn compose_in_memory() -> DesktopApplication<InMemoryTaskRepository> {
    compose(InMemoryTaskRepository::default())
}

pub fn compose_file(root: impl AsRef<Path>) -> DesktopTaskManager {
    TaskManager::new(FileTaskRepository::at(root))
}

/// Desktop-only composition lifecycle. The Tauri entry point delegates state
/// registration here, while the services themselves remain in core/adapters.
pub async fn initialize_desktop_state(app: tauri::AppHandle) -> Result<(), String> {
    let task_directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Failed to initialize task storage: {error}"))?
        .join("tasks");
    app.manage(compose_file(task_directory));

    let operation_store = crate::operations::OperationStore::load(&app)?;
    app.manage(operation_store);

    let watch_state = crate::watch::init_watch_scheduler(&app).await;
    app.manage(watch_state);

    let graph_state = crate::graph::SurrealDbState::initialize(&app).await?;
    crate::history_store::initialize(&graph_state).await?;
    app.manage(graph_state);
    Ok(())
}

pub fn shutdown_desktop(app: &tauri::AppHandle) {
    let history_app_handle = app.clone();
    let _ = tauri::async_runtime::block_on(async move {
        crate::history::cleanup_running_history_on_exit(history_app_handle).await
    });
    let state = app.state::<crate::llm::LlamaState>();
    let status = state.status.blocking_lock();
    if let crate::llm::ModelState::Loading = *status {
        log::info!("Exiting while model is loading; using fast exit to prevent crash.");
        // SAFETY: When terminating the application while llama.cpp is actively loading a large GGUF
        // model in native background C++ threads, standard Rust runtime exit (`std::process::exit`)
        // triggers userspace atexit handlers, TLS destructors, and static C++ destructors.
        // Because llama.cpp C++ loader threads are mid-execution and memory structures are half-initialized,
        // running those destructors causes segmentation faults (SIGSEGV) or heap-corruption crashes.
        // Invoking `_exit(0)` on Unix or `ExitProcess(0)` on Windows bypasses userspace destructors and
        // hands termination directly to the OS kernel, which cleanly reclaims all process memory and handles.
        // This is safe because:
        // 1. All necessary persistent state (such as chat history) has already been flushed in `cleanup_running_history_on_exit` above.
        // 2. The OS kernel unconditionally reclaims all mapped memory, file descriptors, and worker threads.
        #[cfg(unix)]
        unsafe {
            extern "C" {
                fn _exit(status: std::os::raw::c_int) -> !;
            }
            _exit(0);
        }
        #[cfg(windows)]
        unsafe {
            extern "system" {
                fn ExitProcess(uExitCode: u32) -> !;
            }
            ExitProcess(0);
        }
    } else {
        drop(status);
        let mut shared = state.shared.blocking_lock();
        *shared = None;
        log::info!("Llama model cleared on exit.");
    }
}
