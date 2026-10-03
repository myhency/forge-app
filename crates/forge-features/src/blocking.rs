/// Runs blocking work (a `git` process, a folder read) off the UI thread and waits for it without
/// blocking. `None` if the work panicked.
pub async fn run_blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    tokio::task::spawn_blocking(work).await.ok()
}
