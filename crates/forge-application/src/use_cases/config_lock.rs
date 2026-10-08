use std::sync::Arc;

/// One lock for every load-edit-save of the Forge config. Each use case that changes the config holds it from
/// the load to the save, so two of them at once cannot start from the same file and drop each other's change.
/// Clones share the lock; the composition root makes one and gives a clone to each use case.
#[derive(Clone, Default)]
pub struct ConfigLock(Arc<tokio::sync::Mutex<()>>);

impl ConfigLock {
    /// Waits for the lock and holds it until the guard is dropped.
    pub async fn hold(&self) -> tokio::sync::OwnedMutexGuard<()> {
        self.0.clone().lock_owned().await
    }
}
