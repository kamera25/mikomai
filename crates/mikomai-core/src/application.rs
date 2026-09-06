//! Use-case façade kept separate from domain value objects.
use crate::{ApplicationService, TaskRepository, TaskSnapshot};
use uuid::Uuid;
pub fn create_task<R: TaskRepository>(service: &ApplicationService<R>, goal: &str) -> Result<TaskSnapshot, String> { service.start(goal) }
pub fn resume_task<R: TaskRepository>(service: &ApplicationService<R>, id: Uuid) -> Result<Option<TaskSnapshot>, String> { service.resume(id) }
