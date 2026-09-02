use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

#[derive(Clone, Default)]
pub struct BackgroundWorkState {
    inner: Arc<BackgroundWorkInner>,
}

#[derive(Default)]
struct BackgroundWorkInner {
    foreground_queries: AtomicUsize,
    node_refresh_running: AtomicBool,
    foreground_finished: Notify,
}

pub struct ForegroundQueryGuard {
    inner: Arc<BackgroundWorkInner>,
}

pub struct NodeRefreshGuard {
    inner: Arc<BackgroundWorkInner>,
}

impl BackgroundWorkState {
    pub fn begin_foreground_query(&self) -> ForegroundQueryGuard {
        self.inner.foreground_queries.fetch_add(1, Ordering::AcqRel);
        ForegroundQueryGuard {
            inner: self.inner.clone(),
        }
    }

    pub fn try_begin_node_refresh(&self) -> Option<NodeRefreshGuard> {
        self.inner
            .node_refresh_running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| NodeRefreshGuard {
                inner: self.inner.clone(),
            })
    }

    /// Background collectors call this before every device command. A query
    /// arriving between commands therefore jumps ahead of the remaining work.
    pub async fn wait_for_foreground_idle(&self) {
        loop {
            let notified = self.inner.foreground_finished.notified();
            if self.inner.foreground_queries.load(Ordering::Acquire) == 0 {
                return;
            }
            notified.await;
        }
    }
}

impl Drop for ForegroundQueryGuard {
    fn drop(&mut self) {
        if self.inner.foreground_queries.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.inner.foreground_finished.notify_waiters();
        }
    }
}

impl Drop for NodeRefreshGuard {
    fn drop(&mut self) {
        self.inner
            .node_refresh_running
            .store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn foreground_query_blocks_background_until_guard_is_dropped() {
        let state = BackgroundWorkState::default();
        let guard = state.begin_foreground_query();
        let waiting_state = state.clone();
        let waiter = tokio::spawn(async move { waiting_state.wait_for_foreground_idle().await });
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());
        drop(guard);
        waiter.await.unwrap();
    }

    #[test]
    fn only_one_node_refresh_can_run() {
        let state = BackgroundWorkState::default();
        let first = state.try_begin_node_refresh().unwrap();
        assert!(state.try_begin_node_refresh().is_none());
        drop(first);
        assert!(state.try_begin_node_refresh().is_some());
    }
}
