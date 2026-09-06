use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::oneshot;

/// Shared request/response lifecycle for the three interactive choice flows.
pub struct ChoiceBroker {
    pub txs: Mutex<HashMap<String, oneshot::Sender<String>>>,
}

impl ChoiceBroker {
    pub fn new() -> Self {
        Self { txs: Mutex::new(HashMap::new()) }
    }

    pub fn register(&self, id: String) -> Result<oneshot::Receiver<String>, String> {
        let (sender, receiver) = oneshot::channel();
        self.txs.lock().map_err(|_| "Mutex lock poisoned".to_string())?.insert(id, sender);
        Ok(receiver)
    }

    pub fn resolve(&self, id: &str, choice: String) -> Result<(), String> {
        let sender = self.txs.lock().map_err(|_| "Mutex lock poisoned".to_string())?.remove(id);
        if let Some(sender) = sender { let _ = sender.send(choice); }
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
}
