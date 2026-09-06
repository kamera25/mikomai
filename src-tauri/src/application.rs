//! Desktop composition root. Domain behavior is supplied by mikomai-core;
//! this module is intentionally limited to wiring Tauri-owned services.
use mikomai_core::{ApplicationService, TaskRepository};
use mikomai_adapters::memory::InMemoryTaskRepository;

pub type DesktopApplication<R> = ApplicationService<R>;

pub fn compose<R: TaskRepository>(repository: R) -> DesktopApplication<R> {
    ApplicationService { repository }
}

/// Default desktop composition used by development commands and tests.
pub fn compose_in_memory() -> DesktopApplication<InMemoryTaskRepository> {
    compose(InMemoryTaskRepository::default())
}
