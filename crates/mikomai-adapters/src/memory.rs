use mikomai_core::{TaskRepository, TaskSnapshot};
use std::{collections::HashMap, sync::Mutex};
use uuid::Uuid;

#[derive(Default)]
pub struct InMemoryTaskRepository(pub Mutex<HashMap<Uuid, TaskSnapshot>>);
impl TaskRepository for InMemoryTaskRepository {
    fn save(&self, snapshot: &TaskSnapshot) -> Result<(), String> { self.0.lock().map_err(|e| e.to_string())?.insert(snapshot.task.id, snapshot.clone()); Ok(()) }
    fn load(&self, id: Uuid) -> Result<Option<TaskSnapshot>, String> { Ok(self.0.lock().map_err(|e| e.to_string())?.get(&id).cloned()) }
}
