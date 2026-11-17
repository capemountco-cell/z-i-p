//! Running extractions, so the interface can stop one.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct Jobs {
    running: Mutex<HashMap<u64, Arc<AtomicBool>>>,
}

impl Jobs {
    /// Register job `id` and return the flag it should watch.
    pub fn start(&self, id: u64) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.running
            .lock()
            .expect("jobs lock")
            .insert(id, flag.clone());
        flag
    }

    /// Ask job `id` to stop. False if it is not running.
    pub fn cancel(&self, id: u64) -> bool {
        match self.running.lock().expect("jobs lock").get(&id) {
            Some(flag) => {
                flag.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    pub fn finish(&self, id: u64) {
        self.running.lock().expect("jobs lock").remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancels_only_running_jobs() {
        let jobs = Jobs::default();
        let flag = jobs.start(7);
        assert!(jobs.cancel(7));
        assert!(flag.load(Ordering::Relaxed));
        jobs.finish(7);
        assert!(!jobs.cancel(7));
    }
}
