//! Approved operations run on the shared application runtime.
use std::future::Future;

pub(crate) fn submit(
    operation: impl Future<Output = Result<String, String>> + Send + 'static,
    completion: impl FnOnce(Result<String, String>) + Send + 'static,
) -> Result<(), String> {
    let runtime = crate::shared_service().runtime()?;
    let job = runtime.spawn(operation);
    // The task boundary catches Rust panics without poisoning the runtime or
    // leaving Swift's continuation suspended forever.
    runtime.spawn(async move {
        let output = job
            .await
            .unwrap_or_else(|_| Err("approved operation worker failed unexpectedly".into()));
        completion(output);
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn shared_worker_accepts_jobs_inside_another_runtime_and_reports_errors() {
        let caller = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        caller.block_on(async {
            submit(
                async { Err("simulated operation failure".into()) },
                move |result| {
                    tx.send(result).unwrap();
                },
            )
            .unwrap();
        });
        assert!(rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap_err()
            .contains("simulated operation failure"));
        let (tx, rx) = std::sync::mpsc::channel();
        submit(
            async { Ok(std::thread::current().name().unwrap().to_owned()) },
            move |result| {
                tx.send(result).unwrap();
            },
        )
        .unwrap();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap(),
            "mikomai-app-worker"
        );
    }
}
