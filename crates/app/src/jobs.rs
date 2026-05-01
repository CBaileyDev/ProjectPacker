use dashmap::DashMap;
use parking_lot::Mutex;
use projectpacker_core::types::PackResult;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

struct JobEntry {
    /// Retained for future await-on-completion semantics; cancellation goes via `token`.
    #[allow(dead_code)]
    handle: Option<JoinHandle<()>>,
    token: CancellationToken,
}

#[derive(Default)]
pub struct JobRegistry {
    jobs: DashMap<String, Arc<Mutex<JobEntry>>>,
    results: DashMap<String, PackResult>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, job_id: &str, handle: JoinHandle<()>, token: CancellationToken) {
        self.jobs.insert(
            job_id.to_string(),
            Arc::new(Mutex::new(JobEntry {
                handle: Some(handle),
                token,
            })),
        );
    }

    pub fn cancel(&self, job_id: &str) -> bool {
        if let Some(entry) = self.jobs.get(job_id) {
            let guard = entry.lock();
            guard.token.cancel();
            true
        } else {
            false
        }
    }

    pub fn store_result(&self, job_id: &str, result: PackResult) {
        self.results.insert(job_id.to_string(), result);
    }

    pub fn take_result(&self, job_id: &str) -> Option<PackResult> {
        self.results.remove(job_id).map(|(_, v)| v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that cancel() signals the token (not abort on the handle).
    #[test]
    fn cancel_signals_token_not_abort() {
        let registry = JobRegistry::new();
        let token = CancellationToken::new();
        let token_clone = token.clone();

        // Spawn a trivial handle so register() has something to store.
        let rt = tokio::runtime::Runtime::new().unwrap();
        let handle = rt.spawn(async { /* no-op */ });

        registry.register("job-1", handle, token);

        assert!(!token_clone.is_cancelled(), "token should start uncancelled");
        let cancelled = registry.cancel("job-1");
        assert!(cancelled, "cancel() should return true for a known job");
        assert!(token_clone.is_cancelled(), "token must be cancelled after cancel()");
    }
}
