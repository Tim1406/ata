//! Session-side runtime glue for the in-session scheduling tools.
//!
//! `codex-scheduling` owns the pure data (`MonitorRegistry`, `MonitorTask`,
//! etc.) and avoids tokio. The runtime here parks per-monitor tokio
//! `AbortHandle`s next to the registry so `monitor_stop` can actually kill
//! the streaming task that owns the spawned process.

use codex_scheduling::MonitorRegistry;
use codex_scheduling::TaskId;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::task::AbortHandle;

#[derive(Debug)]
pub(crate) struct MonitorRuntime {
    pub registry: Arc<MonitorRegistry>,
    handles: Mutex<HashMap<TaskId, AbortHandle>>,
}

impl MonitorRuntime {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(MonitorRegistry::new()),
            handles: Mutex::new(HashMap::new()),
        }
    }

    pub fn store_handle(&self, id: TaskId, handle: AbortHandle) {
        self.handles
            .lock()
            .expect("MonitorRuntime handles mutex poisoned")
            .insert(id, handle);
    }

    /// Abort the per-monitor task and drop its abort handle. Returns true
    /// if a handle was present. The caller is responsible for separately
    /// removing the entry from the registry data.
    pub fn abort(&self, id: &TaskId) -> bool {
        let mut handles = self
            .handles
            .lock()
            .expect("MonitorRuntime handles mutex poisoned");
        if let Some(handle) = handles.remove(id) {
            handle.abort();
            true
        } else {
            false
        }
    }
}

impl Default for MonitorRuntime {
    fn default() -> Self {
        Self::new()
    }
}
