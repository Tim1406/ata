//! In-memory registry of active `MonitorTask`s.
//!
//! Mirrors the design of `CronRegistry` but for streaming background
//! commands: tool handlers (`monitor_start` / `monitor_list` / `monitor_stop`)
//! mutate it, and the per-monitor tokio task in core/ updates its
//! `lines_emitted` counter as output arrives.
//!
//! Phase 2b: data only — no tokio types live in this crate so it stays
//! decoupled from any runtime. Per-monitor abort handles are tracked by
//! core/ in a parallel map keyed by `TaskId`.

use chrono::DateTime;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::monitor::MonitorTask;
use crate::task::TaskId;
use crate::task::TaskStatus;

#[derive(Debug, Default)]
pub struct MonitorRegistry {
    monitors: Mutex<HashMap<TaskId, MonitorTask>>,
}

impl MonitorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a fully-constructed `MonitorTask`. The caller is responsible
    /// for marking it `Running` and setting `started_at` after the
    /// underlying process is actually spawned.
    pub fn insert(&self, task: MonitorTask) -> TaskId {
        let id = task.id.clone();
        self.monitors
            .lock()
            .expect("MonitorRegistry mutex poisoned")
            .insert(id.clone(), task);
        id
    }

    /// Remove a monitor by id. Returns the removed task if it existed.
    /// Callers should also abort the per-monitor tokio task.
    pub fn remove(&self, id: &TaskId) -> Option<MonitorTask> {
        self.monitors
            .lock()
            .expect("MonitorRegistry mutex poisoned")
            .remove(id)
    }

    /// Snapshot of every registered monitor.
    pub fn list(&self) -> Vec<MonitorTask> {
        self.monitors
            .lock()
            .expect("MonitorRegistry mutex poisoned")
            .values()
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.monitors
            .lock()
            .expect("MonitorRegistry mutex poisoned")
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.monitors
            .lock()
            .expect("MonitorRegistry mutex poisoned")
            .is_empty()
    }

    /// Update bookkeeping when the per-monitor task observes output.
    pub fn record_line(&self, id: &TaskId) {
        let mut monitors = self
            .monitors
            .lock()
            .expect("MonitorRegistry mutex poisoned");
        if let Some(task) = monitors.get_mut(id) {
            task.lines_emitted = task.lines_emitted.saturating_add(1);
        }
    }

    /// Mark a monitor as running (after process spawn succeeded).
    pub fn mark_running(&self, id: &TaskId, started_at: DateTime<Utc>) {
        let mut monitors = self
            .monitors
            .lock()
            .expect("MonitorRegistry mutex poisoned");
        if let Some(task) = monitors.get_mut(id) {
            task.status = TaskStatus::Running;
            task.started_at = Some(started_at);
        }
    }

    /// Mark a monitor terminal: process exited or kill requested. Status
    /// reflects which case. The entry stays in the registry until an
    /// explicit `remove`, so `list` continues to surface it.
    pub fn mark_terminal(&self, id: &TaskId, status: TaskStatus, stopped_at: DateTime<Utc>) {
        let mut monitors = self
            .monitors
            .lock()
            .expect("MonitorRegistry mutex poisoned");
        if let Some(task) = monitors.get_mut(id) {
            task.status = status;
            task.stopped_at = Some(stopped_at);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    #[test]
    fn insert_then_list() {
        let reg = MonitorRegistry::new();
        let id = reg.insert(MonitorTask::new("tail -f log".into()));
        let listed = reg.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, id);
        assert_eq!(listed[0].status, TaskStatus::Pending);
    }

    #[test]
    fn record_line_increments_counter() {
        let reg = MonitorRegistry::new();
        let id = reg.insert(MonitorTask::new("noop".into()));
        reg.record_line(&id);
        reg.record_line(&id);
        let task = reg.list().into_iter().find(|t| t.id == id).unwrap();
        assert_eq!(task.lines_emitted, 2);
    }

    #[test]
    fn mark_running_updates_status_and_started_at() {
        let reg = MonitorRegistry::new();
        let id = reg.insert(MonitorTask::new("noop".into()));
        reg.mark_running(&id, at(100));
        let task = reg.list().into_iter().find(|t| t.id == id).unwrap();
        assert_eq!(task.status, TaskStatus::Running);
        assert_eq!(task.started_at, Some(at(100)));
    }

    #[test]
    fn mark_terminal_records_killed_status() {
        let reg = MonitorRegistry::new();
        let id = reg.insert(MonitorTask::new("noop".into()));
        reg.mark_terminal(&id, TaskStatus::Killed, at(200));
        let task = reg.list().into_iter().find(|t| t.id == id).unwrap();
        assert_eq!(task.status, TaskStatus::Killed);
        assert_eq!(task.stopped_at, Some(at(200)));
    }

    #[test]
    fn remove_drops_the_entry() {
        let reg = MonitorRegistry::new();
        let id = reg.insert(MonitorTask::new("noop".into()));
        assert_eq!(reg.len(), 1);
        let removed = reg.remove(&id);
        assert!(removed.is_some());
        assert_eq!(reg.len(), 0);
    }
}
