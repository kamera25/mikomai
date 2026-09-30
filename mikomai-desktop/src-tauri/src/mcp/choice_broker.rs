use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::oneshot;

/// Shared request/response lifecycle for the three interactive choice flows.
pub struct ChoiceBroker {
    pub txs: Mutex<HashMap<String, oneshot::Sender<String>>>,
}

impl ChoiceBroker {
    pub fn new() -> Self {
        Self {
            txs: Mutex::new(HashMap::new()),
        }
    }

    pub fn register(&self, id: String) -> Result<oneshot::Receiver<String>, String> {
        let (sender, receiver) = oneshot::channel();
        let mut pending = self
            .txs
            .lock()
            .map_err(|_| "Mutex lock poisoned".to_string())?;
        if pending.contains_key(&id) {
            return Err("A choice request with this ID is already pending".to_string());
        }
        pending.insert(id, sender);
        Ok(receiver)
    }

    pub fn resolve(&self, id: &str, choice: String) -> Result<(), String> {
        let sender = self
            .txs
            .lock()
            .map_err(|_| "Mutex lock poisoned".to_string())?
            .remove(id);
        if let Some(sender) = sender {
            let _ = sender.send(choice);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn brokers_a_choice_by_request_id() {
        let broker = ChoiceBroker::new();
        let receiver = broker.register("request-1".into()).unwrap();
        broker.resolve("request-1", "accept".into()).unwrap();
        assert_eq!(receiver.await.unwrap(), "accept");
    }

    #[tokio::test]
    async fn duplicate_request_id_is_rejected_without_replacing_the_first_waiter() {
        let broker = ChoiceBroker::new();
        let first = broker.register("request-1".into()).unwrap();
        assert!(broker.register("request-1".into()).is_err());
        broker.resolve("request-1", "first answer".into()).unwrap();
        assert_eq!(first.await.unwrap(), "first answer");
    }

    #[tokio::test]
    async fn unknown_responses_are_ignored_and_cancelled_receivers_are_removed_on_resolve() {
        let broker = ChoiceBroker::new();
        broker.resolve("unknown", "late answer".into()).unwrap();
        assert!(broker.txs.lock().unwrap().is_empty());

        let receiver = broker.register("cancelled-request".into()).unwrap();
        drop(receiver);
        broker
            .resolve("cancelled-request", "late answer".into())
            .unwrap();
        assert!(broker.txs.lock().unwrap().is_empty());
    }
}
