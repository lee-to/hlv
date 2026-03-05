use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Task lifecycle status
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Awaiting start
    Pending,
    /// Work in progress
    InProgress,
    /// Completed
    Done,
    /// Manually blocked (external reason)
    Blocked,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::InProgress => write!(f, "in_progress"),
            Self::Done => write!(f, "done"),
            Self::Blocked => write!(f, "blocked"),
        }
    }
}

/// Persisted task tracker — lives in milestones.yaml under StageEntry.tasks
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskTracker {
    pub id: String,
    pub status: TaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_reason: Option<String>,
    /// Internal: status before block (for unblock restore). Not client-facing.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "_pre_block_status"
    )]
    pre_block_status: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub meta: HashMap<String, String>,
}

impl TaskTracker {
    /// Create a new tracker in Pending status
    pub fn new(id: String) -> Self {
        Self {
            id,
            status: TaskStatus::Pending,
            started_at: None,
            completed_at: None,
            block_reason: None,
            pre_block_status: None,
            labels: Vec::new(),
            meta: HashMap::new(),
        }
    }

    /// Transition to InProgress. Returns error if already done or blocked.
    pub fn start(&mut self, now: &str) -> anyhow::Result<()> {
        match self.status {
            TaskStatus::Pending => {
                self.status = TaskStatus::InProgress;
                self.started_at = Some(now.to_string());
                Ok(())
            }
            TaskStatus::InProgress => {
                anyhow::bail!("task {} is already in progress", self.id)
            }
            TaskStatus::Done => {
                anyhow::bail!("task {} is already done", self.id)
            }
            TaskStatus::Blocked => {
                anyhow::bail!("task {} is blocked — unblock first", self.id)
            }
        }
    }

    /// Transition to Done. Returns error if not in progress.
    pub fn done(&mut self, now: &str) -> anyhow::Result<()> {
        match self.status {
            TaskStatus::InProgress => {
                self.status = TaskStatus::Done;
                self.completed_at = Some(now.to_string());
                Ok(())
            }
            TaskStatus::Done => {
                anyhow::bail!("task {} is already done", self.id)
            }
            TaskStatus::Pending => {
                anyhow::bail!("task {} has not been started yet", self.id)
            }
            TaskStatus::Blocked => {
                anyhow::bail!("task {} is blocked — unblock first", self.id)
            }
        }
    }

    /// Block with a reason. Can block from Pending or InProgress.
    pub fn block(&mut self, reason: &str) -> anyhow::Result<()> {
        match self.status {
            TaskStatus::Pending | TaskStatus::InProgress => {
                self.pre_block_status = Some(self.status.to_string());
                self.status = TaskStatus::Blocked;
                self.block_reason = Some(reason.to_string());
                Ok(())
            }
            TaskStatus::Blocked => {
                anyhow::bail!("task {} is already blocked", self.id)
            }
            TaskStatus::Done => {
                anyhow::bail!("task {} is already done — cannot block", self.id)
            }
        }
    }

    /// Unblock — restore previous status (Pending or InProgress).
    pub fn unblock(&mut self) -> anyhow::Result<()> {
        if self.status != TaskStatus::Blocked {
            anyhow::bail!("task {} is not blocked", self.id);
        }
        let was_in_progress = self
            .pre_block_status
            .take()
            .map(|s| s == "in_progress")
            .unwrap_or(false);
        self.status = if was_in_progress {
            TaskStatus::InProgress
        } else {
            TaskStatus::Pending
        };
        self.block_reason = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_status_serde() {
        let json = serde_json::to_string(&TaskStatus::InProgress).unwrap();
        assert_eq!(json, "\"in_progress\"");

        let status: TaskStatus = serde_json::from_str("\"blocked\"").unwrap();
        assert_eq!(status, TaskStatus::Blocked);
    }

    #[test]
    fn task_tracker_serde_roundtrip() {
        let tracker = TaskTracker {
            id: "TASK-001".to_string(),
            status: TaskStatus::InProgress,
            started_at: Some("2026-03-08T10:00:00Z".to_string()),
            completed_at: None,
            block_reason: None,
            pre_block_status: None,
            labels: vec!["frontend".to_string()],
            meta: HashMap::from([("priority".to_string(), "high".to_string())]),
        };

        let yaml = serde_yaml::to_string(&tracker).unwrap();
        let restored: TaskTracker = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(restored.id, "TASK-001");
        assert_eq!(restored.status, TaskStatus::InProgress);
        assert_eq!(restored.labels, vec!["frontend"]);
        assert_eq!(restored.meta.get("priority").unwrap(), "high");
    }

    #[test]
    fn task_tracker_minimal_serde() {
        // Minimal YAML — only required fields
        let yaml = "id: TASK-001\nstatus: pending\n";
        let tracker: TaskTracker = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(tracker.id, "TASK-001");
        assert_eq!(tracker.status, TaskStatus::Pending);
        assert!(tracker.labels.is_empty());
        assert!(tracker.meta.is_empty());
        assert!(tracker.started_at.is_none());
    }

    #[test]
    fn lifecycle_start_done() {
        let mut t = TaskTracker::new("TASK-001".to_string());
        assert_eq!(t.status, TaskStatus::Pending);

        t.start("2026-03-08T10:00:00Z").unwrap();
        assert_eq!(t.status, TaskStatus::InProgress);
        assert!(t.started_at.is_some());

        t.done("2026-03-08T11:00:00Z").unwrap();
        assert_eq!(t.status, TaskStatus::Done);
        assert!(t.completed_at.is_some());
    }

    #[test]
    fn lifecycle_block_unblock_from_pending() {
        let mut t = TaskTracker::new("TASK-001".to_string());

        t.block("waiting for API access").unwrap();
        assert_eq!(t.status, TaskStatus::Blocked);
        assert_eq!(t.block_reason.as_deref(), Some("waiting for API access"));

        t.unblock().unwrap();
        assert_eq!(t.status, TaskStatus::Pending);
        assert!(t.block_reason.is_none());
    }

    #[test]
    fn lifecycle_block_unblock_from_in_progress() {
        let mut t = TaskTracker::new("TASK-001".to_string());
        t.start("2026-03-08T10:00:00Z").unwrap();

        t.block("server down").unwrap();
        assert_eq!(t.status, TaskStatus::Blocked);

        t.unblock().unwrap();
        assert_eq!(t.status, TaskStatus::InProgress);
    }

    #[test]
    fn cannot_start_done_task() {
        let mut t = TaskTracker::new("TASK-001".to_string());
        t.start("now").unwrap();
        t.done("now").unwrap();
        assert!(t.start("now").is_err());
    }

    #[test]
    fn cannot_block_done_task() {
        let mut t = TaskTracker::new("TASK-001".to_string());
        t.start("now").unwrap();
        t.done("now").unwrap();
        assert!(t.block("reason").is_err());
    }

    #[test]
    fn cannot_done_pending_task() {
        let mut t = TaskTracker::new("TASK-001".to_string());
        assert!(t.done("now").is_err());
    }

    #[test]
    fn cannot_unblock_non_blocked() {
        let mut t = TaskTracker::new("TASK-001".to_string());
        assert!(t.unblock().is_err());
    }
}
