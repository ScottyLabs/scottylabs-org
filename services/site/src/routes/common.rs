//! Small helpers the route modules share.

use crate::error::{AppError, AppResult};

/// Runs CPU-bound work, such as sanitizing rich text, on the blocking thread pool, so a slow request never stalls the others that share its async worker thread.
pub async fn blocking<T>(work: impl FnOnce() -> T + Send + 'static) -> AppResult<T>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!("a blocking task failed: {error}")))
}
