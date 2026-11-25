//! In-memory registry of watchable background tasks.
//!
//! Deliberately separate from the plugin job tracker
//! (`plugin_manager::executor::ActiveJobTracker`): that one is bound to plugin
//! ids, plugin result types and `plugin://` events. This registry only stores
//! [`TaskSnapshot`]s plus a cooperative cancel flag per task; the code that
//! does the work reads the flag between items.
//!
//! The registry is registered as its own Tauri managed state (not an
//! `AppState` field) and is cheap to clone — clones share one table.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use chrono::Utc;
use uuid::Uuid;

use crate::models::background_task::{TaskKind, TaskSnapshot, TaskState};
use crate::plugin_manager::error::CommandError;

struct Entry {
    snapshot: TaskSnapshot,
    cancel: Arc<AtomicBool>,
    /// Registration order, so `list` is stable for equal timestamps.
    seq: u64,
}

#[derive(Default)]
struct Table {
    tasks: HashMap<String, Entry>,
    next_seq: u64,
}

/// Shared table of background tasks. Clones refer to the same table.
#[derive(Clone, Default)]
pub struct BackgroundTasks {
    table: Arc<Mutex<Table>>,
}

/// What a caller needs to run a freshly registered task.
pub struct StartedTask {
    pub snapshot: TaskSnapshot,
    pub cancel: Arc<AtomicBool>,
}

fn not_found(task_id: &str) -> CommandError {
    CommandError {
        code: "TASK_NOT_FOUND".to_string(),
        message: format!("background task '{task_id}' not found"),
    }
}

impl BackgroundTasks {
    fn table(&self) -> MutexGuard<'_, Table> {
        // The table holds plain data and every critical section is a few
        // field writes, so a poisoned lock still holds a usable table.
        self.table
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Registers a running task, unless one of the same `kind` is already live
    /// for the same session — then returns `None`.
    ///
    /// The check and the insert happen under one lock, so two imports that
    /// finish at the same moment cannot both start a task for the session.
    pub fn try_start(
        &self,
        kind: TaskKind,
        title: &str,
        session_id: &str,
        session_name: &str,
        total: usize,
        skipped: usize,
    ) -> Option<StartedTask> {
        let mut table = self.table();
        let duplicate = table.tasks.values().any(|entry| {
            entry.snapshot.kind == kind
                && entry.snapshot.session_id == session_id
                && entry.snapshot.state.is_live()
        });
        if duplicate {
            return None;
        }
        let snapshot = TaskSnapshot {
            id: Uuid::new_v4().to_string(),
            kind,
            title: title.to_string(),
            session_id: session_id.to_string(),
            session_name: session_name.to_string(),
            state: TaskState::Running,
            done: 0,
            total,
            unscorable: 0,
            skipped,
            current: None,
            error: None,
            started_at: Utc::now().to_rfc3339(),
            finished_at: None,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let seq = table.next_seq;
        table.next_seq += 1;
        table.tasks.insert(
            snapshot.id.clone(),
            Entry {
                snapshot: snapshot.clone(),
                cancel: Arc::clone(&cancel),
                seq,
            },
        );
        Some(StartedTask { snapshot, cancel })
    }

    /// Applies `change` to the task and returns the new snapshot, or `None`
    /// when the task no longer exists.
    pub fn update(
        &self,
        task_id: &str,
        change: impl FnOnce(&mut TaskSnapshot),
    ) -> Option<TaskSnapshot> {
        let mut table = self.table();
        let entry = table.tasks.get_mut(task_id)?;
        change(&mut entry.snapshot);
        Some(entry.snapshot.clone())
    }

    /// Moves the task to a final `state` and stamps `finished_at`.
    pub fn finish(
        &self,
        task_id: &str,
        state: TaskState,
        error: Option<String>,
    ) -> Option<TaskSnapshot> {
        self.update(task_id, |snapshot| {
            snapshot.state = state;
            snapshot.error = error;
            snapshot.finished_at = Some(Utc::now().to_rfc3339());
        })
    }

    /// Every task, live ones first, newest first within each group.
    pub fn list(&self) -> Vec<TaskSnapshot> {
        let table = self.table();
        let mut entries: Vec<&Entry> = table.tasks.values().collect();
        entries.sort_by(|a, b| {
            b.snapshot
                .state
                .is_live()
                .cmp(&a.snapshot.state.is_live())
                .then(b.seq.cmp(&a.seq))
        });
        entries.into_iter().map(|e| e.snapshot.clone()).collect()
    }

    /// Asks a task to stop. The worker notices the flag between items, so the
    /// task keeps its `running` state until the worker returns and calls
    /// [`BackgroundTasks::finish`]. Cancelling a finished task is a no-op.
    ///
    /// # Errors
    ///
    /// `TASK_NOT_FOUND` when no task has this id.
    pub fn cancel(&self, task_id: &str) -> Result<(), CommandError> {
        let table = self.table();
        let entry = table.tasks.get(task_id).ok_or_else(|| not_found(task_id))?;
        if entry.snapshot.state.is_live() {
            entry.cancel.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Forgets every finished task, keeping the live ones. Returns the ids
    /// that were removed.
    pub fn clear_finished(&self) -> Vec<String> {
        let mut table = self.table();
        let removed: Vec<String> = table
            .tasks
            .iter()
            .filter(|(_, entry)| !entry.snapshot.state.is_live())
            .map(|(id, _)| id.clone())
            .collect();
        for id in &removed {
            table.tasks.remove(id);
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start(tasks: &BackgroundTasks, session: &str) -> Option<StartedTask> {
        tasks.try_start(TaskKind::BlurScoring, "Menilai", session, "Sesi", 5, 0)
    }

    #[test]
    fn a_second_live_task_for_the_same_session_is_refused() {
        let tasks = BackgroundTasks::default();
        let first = start(&tasks, "s1").expect("first starts");
        assert!(start(&tasks, "s1").is_none());
        assert!(
            start(&tasks, "s2").is_some(),
            "other sessions are unaffected"
        );

        tasks.finish(&first.snapshot.id, TaskState::Done, None);
        assert!(
            start(&tasks, "s1").is_some(),
            "a finished task no longer blocks the session"
        );
    }

    #[test]
    fn finish_stamps_the_end_and_keeps_the_error() {
        let tasks = BackgroundTasks::default();
        let started = start(&tasks, "s1").expect("start");
        assert_eq!(started.snapshot.state, TaskState::Running);
        assert!(started.snapshot.finished_at.is_none());

        let done = tasks
            .finish(&started.snapshot.id, TaskState::Failed, Some("boom".into()))
            .expect("known task");
        assert_eq!(done.state, TaskState::Failed);
        assert_eq!(done.error.as_deref(), Some("boom"));
        assert!(done.finished_at.is_some());
    }

    #[test]
    fn cancel_sets_the_flag_only_for_live_tasks() {
        let tasks = BackgroundTasks::default();
        let started = start(&tasks, "s1").expect("start");
        tasks.cancel(&started.snapshot.id).expect("live task");
        assert!(started.cancel.load(Ordering::Relaxed));

        let finished = start(&tasks, "s2").expect("start");
        tasks.finish(&finished.snapshot.id, TaskState::Done, None);
        tasks
            .cancel(&finished.snapshot.id)
            .expect("no-op, not an error");
        assert!(!finished.cancel.load(Ordering::Relaxed));
    }

    #[test]
    fn cancel_of_an_unknown_task_is_an_error() {
        let err = BackgroundTasks::default()
            .cancel("nope")
            .expect_err("unknown id");
        assert_eq!(err.code, "TASK_NOT_FOUND");
    }

    #[test]
    fn clear_finished_keeps_live_tasks_and_list_puts_them_first() {
        let tasks = BackgroundTasks::default();
        let old = start(&tasks, "s1").expect("start");
        tasks.finish(&old.snapshot.id, TaskState::Done, None);
        let live = start(&tasks, "s2").expect("start");

        let listed = tasks.list();
        assert_eq!(listed[0].id, live.snapshot.id, "live first");
        assert_eq!(listed[1].id, old.snapshot.id);

        assert_eq!(tasks.clear_finished(), vec![old.snapshot.id]);
        let remaining = tasks.list();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, live.snapshot.id);
    }
}
