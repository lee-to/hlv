use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

/// Version 1 evidence contract. Producers supply outcomes; HLV never synthesizes them.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ExecutionEvidenceFile {
    pub schema_version: u32,
    pub runs: Vec<ExecutionEvidenceRun>,
}

impl ExecutionEvidenceFile {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        Ok(serde_yaml::from_str(&std::fs::read_to_string(path)?)?)
    }
}

/// Snapshot captured BEFORE the external runner executes.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExecutionSnapshot {
    pub binding: String,
    pub requirement: String,
    pub approved_requirement_revision: String,
    pub test: String,
    pub gate: String,
    /// Repo-relative regular-file paths mapped to lowercase SHA-256 hashes.
    pub inputs: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ExecutionEvidenceRun {
    pub snapshot: ExecutionSnapshot,
    pub code_revision: String,
    pub run_id: String,
    pub outcome: ExecutionOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<FixedOffset>>,
    /// A runner report/observation URL or artifact reference. Never fetched by HLV.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_ref: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionOutcome {
    Passed,
    Failed,
    Incomplete,
    Skipped,
}
