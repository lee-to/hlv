use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::task::TaskTracker;

/// Root of milestones.yaml
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct MilestoneMap {
    pub project: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<MilestoneCurrent>,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
}

/// Current active milestone
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct MilestoneCurrent {
    pub id: String,
    pub number: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<u32>,
    #[serde(default)]
    pub stages: Vec<StageEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gate_results: Vec<GateResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<MilestoneGitConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub meta: HashMap<String, String>,
}

/// Per-milestone git config override
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct MilestoneGitConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_per_milestone: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub squash_on_merge: Option<bool>,
}

/// Result of a gate run, stored in milestones.yaml
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct GateResult {
    pub id: String,
    pub status: GateRunStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_at: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GateRunStatus {
    Passed,
    Failed,
    Skipped,
}

impl std::fmt::Display for GateRunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Passed => write!(f, "passed"),
            Self::Failed => write!(f, "failed"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}

/// Stage metadata in milestones.yaml
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct StageEntry {
    pub id: u32,
    pub scope: String,
    pub status: StageStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<TaskTracker>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub meta: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    Pending,
    Verified,
    Implementing,
    Implemented,
    Validating,
    Validated,
}

impl std::fmt::Display for StageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Verified => write!(f, "verified"),
            Self::Implementing => write!(f, "implementing"),
            Self::Implemented => write!(f, "implemented"),
            Self::Validating => write!(f, "validating"),
            Self::Validated => write!(f, "validated"),
        }
    }
}

/// Completed milestone in history
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct HistoryEntry {
    pub id: String,
    pub number: u32,
    pub status: MilestoneStatus,
    #[serde(default)]
    pub contracts: Vec<ContractChange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_at: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MilestoneStatus {
    Merged,
    Aborted,
}

impl std::fmt::Display for MilestoneStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Merged => write!(f, "merged"),
            Self::Aborted => write!(f, "aborted"),
        }
    }
}

/// Contract created or modified by a milestone
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ContractChange {
    pub name: String,
    pub action: ContractChangeAction,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContractChangeAction {
    Created,
    Modified,
}

impl MilestoneMap {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let map: MilestoneMap = serde_yaml::from_str(&content)?;
        Ok(map)
    }

    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let yaml = serde_yaml::to_string(self)?;
        let content = format!(
            "# yaml-language-server: $schema=schema/milestones-schema.json\n{}",
            yaml
        );
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Next milestone number (max of current + history + 1)
    pub fn next_number(&self) -> u32 {
        let current_num = self.current.as_ref().map(|c| c.number).unwrap_or(0);
        let history_max = self.history.iter().map(|h| h.number).max().unwrap_or(0);
        current_num.max(history_max) + 1
    }

    /// Resolve current contract version by walking history.
    /// Returns the path to the milestone directory containing the latest version.
    pub fn resolve_contract(&self, name: &str) -> Option<ResolvedContract> {
        // Check current milestone first
        if let Some(current) = &self.current {
            // Current milestone's contracts are always the most recent
            // (caller checks if the file actually exists)
            return Some(ResolvedContract {
                milestone_id: current.id.clone(),
                milestone_number: current.number,
            });
        }

        // Walk history in reverse (most recent first)
        for entry in self.history.iter().rev() {
            if entry.status != MilestoneStatus::Merged {
                continue;
            }
            for c in &entry.contracts {
                if c.name == name {
                    return Some(ResolvedContract {
                        milestone_id: entry.id.clone(),
                        milestone_number: entry.number,
                    });
                }
            }
        }

        None
    }
}

/// Result of contract version resolution
#[derive(Debug, Clone)]
pub struct ResolvedContract {
    pub milestone_id: String,
    pub milestone_number: u32,
}
