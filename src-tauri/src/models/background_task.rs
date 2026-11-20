//! Shared model for watchable background tasks.
//!
//! Crosses the IPC boundary: the registry in
//! [`crate::services::background_tasks`] hands these snapshots to the
//! frontend both as command results and as `background-task-updated` events.

use serde::{Deserialize, Serialize};

/// Event emitted whenever a background task changes. Payload: [`TaskSnapshot`].
pub const BACKGROUND_TASK_EVENT: &str = "background-task-updated";

/// What a task does. Lets the UI pick a title and wording per kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    /// Scoring the sharpness of imported images.
    BlurScoring,
}

/// Where a task is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    /// Registered, not started yet.
    Queued,
    Running,
    /// Finished and everything worked.
    Done,
    /// Finished, but some items could not be processed.
    Warn,
    /// Stopped by the user; unfinished items are picked up again on the next
    /// launch.
    Cancelled,
    /// Stopped by an error.
    Failed,
}

impl TaskState {
    /// Whether the task is still queued or running.
    pub fn is_live(self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }
}

/// A point-in-time view of one background task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSnapshot {
    pub id: String,
    pub kind: TaskKind,
    pub title: String,
    pub session_id: String,
    pub session_name: String,
    pub state: TaskState,
    /// Items finished so far.
    pub done: usize,
    pub total: usize,
    /// Finished items that produced no result (for blur scoring: too small or
    /// undecodable).
    pub unscorable: usize,
    /// Items deliberately not processed at all, outside of `total` (for blur
    /// scoring: standalone DNGs with no paired JPEG — their sharpness is
    /// never scored, unlike `unscorable`, which counts items that were
    /// attempted and failed).
    pub skipped: usize,
    /// Name of the item that most recently finished, if any.
    pub current: Option<String>,
    /// Failure message when `state` is `failed`.
    pub error: Option<String>,
    /// RFC 3339, UTC.
    pub started_at: String,
    /// RFC 3339, UTC. `None` while the task is live.
    pub finished_at: Option<String>,
}
