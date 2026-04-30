use dashmap::DashMap;
use parking_lot::Mutex;
use projectpacker_core::types::PackResult;
use std::sync::Arc;
use tokio::task::JoinHandle;

#[derive(Default)]
pub struct JobRegistry {
    handles: DashMap<String, Arc<Mutex<Option<JoinHandle<()>>>>>,
    results: DashMap<String, PackResult>,
}

impl JobRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&self, job_id: &str, handle: JoinHandle<()>) {
        self.handles.insert(job_id.to_string(), Arc::new(Mutex::new(Some(handle))));
    }

    pub fn cancel(&self, job_id: &str) -> bool {
        if let Some(entry) = self.handles.get(job_id) {
            if let Some(h) = entry.lock().take() { h.abort(); return true; }
        }
        false
    }

    pub fn store_result(&self, job_id: &str, result: PackResult) {
        self.results.insert(job_id.to_string(), result);
    }

    pub fn take_result(&self, job_id: &str) -> Option<PackResult> {
        self.results.remove(job_id).map(|(_, v)| v)
    }
}
