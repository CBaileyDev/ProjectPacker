//! Job registry — tracks per-pack cancellation tokens and stores results.
//!
//! Cancellation goes through `CancellationToken`, not `JoinHandle::abort()`
//! (which can't interrupt a blocking thread anyway). The registry holds the
//! token from the time `pack_start` registers it, through pack execution,
//! until the result is stored (success) or the job is discarded (error).
//! See `commands::pack_start` for the registration ordering rationale.

use dashmap::DashMap;
use projectpacker_core::types::PackResult;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct JobRegistry {
    tokens: DashMap<String, CancellationToken>,
    results: DashMap<String, PackResult>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a job's cancellation token. Must be called BEFORE the
    /// pack task is spawned so a fast-completing task can't race past
    /// registration and leak its token (if registration happened later
    /// than result-storage, the token would never be evicted).
    pub fn register(&self, job_id: &str, token: CancellationToken) {
        self.tokens.insert(job_id.to_string(), token);
    }

    /// Signal cancellation. Returns true if the job exists.
    pub fn cancel(&self, job_id: &str) -> bool {
        match self.tokens.get(job_id) {
            Some(t) => {
                t.cancel();
                true
            }
            None => false,
        }
    }

    /// Success path: evict the token and store the result.
    pub fn store_result(&self, job_id: &str, result: PackResult) {
        self.tokens.remove(job_id);
        self.results.insert(job_id.to_string(), result);
    }

    /// Error path: evict the token without storing a result.
    /// Prevents token leakage when pack() fails before producing a PackResult.
    pub fn discard(&self, job_id: &str) {
        self.tokens.remove(job_id);
    }

    pub fn take_result(&self, job_id: &str) -> Option<PackResult> {
        self.results.remove(job_id).map(|(_, v)| v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_signals_token() {
        let registry = JobRegistry::new();
        let token = CancellationToken::new();
        let observer = token.clone();

        registry.register("job-1", token);

        assert!(!observer.is_cancelled());
        assert!(registry.cancel("job-1"));
        assert!(observer.is_cancelled());
    }

    #[test]
    fn cancel_returns_false_for_unknown_job() {
        let registry = JobRegistry::new();
        assert!(!registry.cancel("does-not-exist"));
    }

    #[test]
    fn store_result_evicts_token() {
        let registry = JobRegistry::new();
        let token = CancellationToken::new();
        registry.register("job-1", token);

        let result = PackResult {
            output: "x".into(),
            claude_code_prompt: String::new(),
            stats: projectpacker_core::types::PackStats {
                files_total: 0,
                files_included: 0,
                files_skipped: 0,
                bytes_total: 0,
                tokens_total: None,
                tokens_per_model: None,
                secrets_found: 0,
                duration_ms: 0,
            },
            warnings: Vec::new(),
            redactions: Vec::new(),
        };
        registry.store_result("job-1", result);

        // Token should be evicted (calling cancel returns false now).
        assert!(!registry.cancel("job-1"));
    }

    #[test]
    fn discard_evicts_token_without_result() {
        let registry = JobRegistry::new();
        registry.register("job-1", CancellationToken::new());
        registry.discard("job-1");
        assert!(!registry.cancel("job-1"));
        assert!(registry.take_result("job-1").is_none());
    }
}
