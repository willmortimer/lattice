//! Idle shutdown when no clients are connected and keep-running is disabled.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

/// Tracks live client connections and triggers idle shutdown when configured.
pub struct ConnectionTracker {
    active: AtomicUsize,
    keep_services_running: bool,
    idle_timeout: Duration,
    idle_task: Mutex<Option<JoinHandle<()>>>,
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
}

impl ConnectionTracker {
    pub fn new(
        keep_services_running: bool,
        idle_timeout: Duration,
        shutdown: oneshot::Sender<()>,
    ) -> Arc<Self> {
        Arc::new(Self {
            active: AtomicUsize::new(0),
            keep_services_running,
            idle_timeout,
            idle_task: Mutex::new(None),
            shutdown: Mutex::new(Some(shutdown)),
        })
    }

    /// Call after a client handshake succeeds.
    pub async fn on_connect(self: &Arc<Self>) {
        self.cancel_idle_timer().await;
        self.active.fetch_add(1, Ordering::SeqCst);
    }

    /// RAII guard that decrements the active count on drop.
    pub fn guard(self: &Arc<Self>) -> ConnectionGuard {
        ConnectionGuard {
            tracker: Arc::clone(self),
        }
    }
}

pub struct ConnectionGuard {
    tracker: Arc<ConnectionTracker>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        let prev = self.tracker.active.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 && !self.tracker.keep_services_running {
            let tracker = Arc::clone(&self.tracker);
            tokio::spawn(async move {
                tracker.schedule_idle_shutdown().await;
            });
        }
    }
}

impl ConnectionTracker {
    async fn cancel_idle_timer(&self) {
        if let Some(handle) = self.idle_task.lock().await.take() {
            handle.abort();
        }
    }

    async fn schedule_idle_shutdown(self: &Arc<Self>) {
        self.cancel_idle_timer().await;
        let this = Arc::clone(self);
        let handle = tokio::spawn(async move {
            tokio::time::sleep(this.idle_timeout).await;
            if this.active.load(Ordering::SeqCst) == 0 {
                if let Some(tx) = this.shutdown.lock().await.take() {
                    let _ = tx.send(());
                }
            }
        });
        *self.idle_task.lock().await = Some(handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration as TokioDuration};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn idle_shutdown_fires_when_last_client_disconnects() {
        let (tx, mut rx) = oneshot::channel();
        let tracker = ConnectionTracker::new(false, Duration::from_millis(50), tx);
        tracker.on_connect().await;
        drop(tracker.guard());
        tokio::time::timeout(TokioDuration::from_secs(2), &mut rx)
            .await
            .expect("idle shutdown should fire")
            .expect("channel open");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn keep_running_skips_idle_shutdown() {
        let (tx, mut rx) = oneshot::channel();
        let tracker = ConnectionTracker::new(true, Duration::from_millis(50), tx);
        tracker.on_connect().await;
        drop(tracker.guard());
        sleep(TokioDuration::from_millis(150)).await;
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn reconnect_cancels_pending_idle_shutdown() {
        let (tx, mut rx) = oneshot::channel();
        let tracker = ConnectionTracker::new(false, Duration::from_millis(80), tx);
        tracker.on_connect().await;
        drop(tracker.guard());
        sleep(TokioDuration::from_millis(30)).await;
        tracker.on_connect().await;
        drop(tracker.guard());
        sleep(TokioDuration::from_millis(30)).await;
        tracker.on_connect().await;
        drop(tracker.guard());
        tokio::time::timeout(TokioDuration::from_secs(2), &mut rx)
            .await
            .expect("idle shutdown after final disconnect")
            .expect("channel open");
    }
}
