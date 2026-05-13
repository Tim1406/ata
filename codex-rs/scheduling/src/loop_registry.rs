//! In-memory registry of active `LoopTask`s.
//!
//! Same pattern as `MonitorRegistry`: data only. Per-loop tokio abort
//! handles live in `core/scheduling_runtime.rs`. Each loop has its own
//! task (one tokio timer per loop) so killing one doesn't disturb others.

use chrono::DateTime;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::loop_task::LoopTask;
use crate::task::TaskId;
use crate::task::TaskStatus;

#[derive(Debug, Default)]
pub struct LoopRegistry {
    loops: Mutex<HashMap<TaskId, LoopTask>>,
}

impl LoopRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, task: LoopTask) -> TaskId {
        let id = task.id.clone();
        self.loops
            .lock()
            .expect("LoopRegistry mutex poisoned")
            .insert(id.clone(), task);
        id
    }

    pub fn remove(&self, id: &TaskId) -> Option<LoopTask> {
        self.loops
            .lock()
            .expect("LoopRegistry mutex poisoned")
            .remove(id)
    }

    pub fn list(&self) -> Vec<LoopTask> {
        self.loops
            .lock()
            .expect("LoopRegistry mutex poisoned")
            .values()
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.loops
            .lock()
            .expect("LoopRegistry mutex poisoned")
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.loops
            .lock()
            .expect("LoopRegistry mutex poisoned")
            .is_empty()
    }

    /// Current status of a loop, or `None` if it has been removed.
    pub fn status(&self, id: &TaskId) -> Option<TaskStatus> {
        self.loops
            .lock()
            .expect("LoopRegistry mutex poisoned")
            .get(id)
            .map(|task| task.status)
    }

    /// Record one iteration of a loop. Called by the per-loop tokio task
    /// each time it fires.
    pub fn record_iteration(&self, id: &TaskId, fired_at: DateTime<Utc>) {
        let mut loops = self.loops.lock().expect("LoopRegistry mutex poisoned");
        if let Some(task) = loops.get_mut(id) {
            task.last_iter_at = Some(fired_at);
            task.iteration_count = task.iteration_count.saturating_add(1);
            // Return to `Pending` between firings so `/scheduling` reads as
            // "waiting for next interval tick" rather than stuck on `Running`
            // forever. Mirrors the cron registry fix. Terminal transitions
            // (Completed / Killed) come from `mark_terminal` via loop_stop.
            task.status = TaskStatus::Pending;
        }
    }

    /// Mark a loop terminal (stopped by user or completed naturally).
    pub fn mark_terminal(&self, id: &TaskId, status: TaskStatus, stopped_at: DateTime<Utc>) {
        let mut loops = self.loops.lock().expect("LoopRegistry mutex poisoned");
        if let Some(task) = loops.get_mut(id) {
            task.status = status;
            task.last_iter_at = Some(stopped_at);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::time::Duration;

    fn at(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    #[test]
    fn insert_then_list() {
        let reg = LoopRegistry::new();
        let id = reg.insert(LoopTask::new_fixed("poll".into(), Duration::from_secs(60)));
        assert_eq!(reg.len(), 1);
        assert_eq!(reg.list()[0].id, id);
    }

    #[test]
    fn record_iteration_increments_counter() {
        let reg = LoopRegistry::new();
        let id = reg.insert(LoopTask::new_fixed("p".into(), Duration::from_secs(10)));
        reg.record_iteration(&id, at(100));
        reg.record_iteration(&id, at(200));
        let task = reg.list().into_iter().find(|t| t.id == id).unwrap();
        assert_eq!(task.iteration_count, 2);
        assert_eq!(task.last_iter_at, Some(at(200)));
        // Between iterations the loop returns to Pending ("waiting for next
        // interval tick"); a terminal transition only happens via
        // mark_terminal (loop_stop).
        assert_eq!(task.status, TaskStatus::Pending);
    }

    #[test]
    fn mark_terminal_records_killed() {
        let reg = LoopRegistry::new();
        let id = reg.insert(LoopTask::new_fixed("p".into(), Duration::from_secs(10)));
        reg.mark_terminal(&id, TaskStatus::Killed, at(500));
        let task = reg.list().into_iter().find(|t| t.id == id).unwrap();
        assert_eq!(task.status, TaskStatus::Killed);
    }

    #[test]
    fn status_returns_none_when_missing_and_tracks_terminal() {
        let reg = LoopRegistry::new();
        let unknown = crate::task::TaskId::new();
        assert!(reg.status(&unknown).is_none());
        let id = reg.insert(LoopTask::new_fixed("p".into(), Duration::from_secs(10)));
        assert_eq!(reg.status(&id), Some(TaskStatus::Pending));
        reg.mark_terminal(&id, TaskStatus::Completed, at(1));
        assert_eq!(reg.status(&id), Some(TaskStatus::Completed));
    }

    #[test]
    fn remove_drops_entry() {
        let reg = LoopRegistry::new();
        let id = reg.insert(LoopTask::new_fixed("p".into(), Duration::from_secs(10)));
        assert!(reg.remove(&id).is_some());
        assert_eq!(reg.len(), 0);
    }
}
