use anyhow::{Context, Result};
use colored::Colorize;
use std::path::Path;

use super::style;
use crate::model::artifact::{ArtifactFull, ArtifactIndex};
use crate::model::milestone::MilestoneMap;

/// `hlv artifacts [--global | --milestone] [--json]`
pub fn run_list(root: &Path, global_only: bool, milestone_only: bool, json: bool) -> Result<()> {
    let milestone_id = current_milestone_id(root);

    let index = if global_only {
        ArtifactIndex {
            global: ArtifactIndex::load_global(root)?,
            milestone: Vec::new(),
        }
    } else if milestone_only {
        let mid = milestone_id.as_deref().context("No active milestone")?;
        ArtifactIndex {
            global: Vec::new(),
            milestone: ArtifactIndex::load_milestone(root, mid)?,
        }
    } else {
        ArtifactIndex::load(root, milestone_id.as_deref())?
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&index)?);
    } else {
        if !index.global.is_empty() {
            style::header("Global artifacts");
            for a in &index.global {
                println!("  {} {} ({})", "·".dimmed(), a.name.bold(), a.kind);
            }
        }
        if !index.milestone.is_empty() {
            style::header("Milestone artifacts");
            for a in &index.milestone {
                println!("  {} {} ({})", "·".dimmed(), a.name.bold(), a.kind);
            }
        }
        if index.global.is_empty() && index.milestone.is_empty() {
            style::hint("No artifacts found.");
        }
    }
    Ok(())
}

/// `hlv artifacts show <name> [--global | --milestone] [--json]`
pub fn run_show(
    root: &Path,
    name: &str,
    global_only: bool,
    milestone_only: bool,
    json: bool,
) -> Result<()> {
    let milestone_id = current_milestone_id(root);

    // Search in appropriate scope
    let artifact = if global_only {
        find_artifact(root, name, true, None)?
    } else if milestone_only {
        let mid = milestone_id.as_deref().context("No active milestone")?;
        find_artifact(root, name, false, Some(mid))?
    } else {
        // Try milestone first, then global
        let mid = milestone_id.as_deref();
        if let Some(mid) = mid {
            find_artifact(root, name, false, Some(mid))
                .or_else(|_| find_artifact(root, name, true, None))?
        } else {
            find_artifact(root, name, true, None)?
        }
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&artifact)?);
    } else {
        println!("{}", artifact.content);
    }
    Ok(())
}

/// Returns artifact index as data (no stdout).
pub fn get_artifacts_list(
    root: &Path,
    global_only: bool,
    milestone_only: bool,
) -> Result<ArtifactIndex> {
    let milestone_id = current_milestone_id(root);

    if global_only {
        Ok(ArtifactIndex {
            global: ArtifactIndex::load_global(root)?,
            milestone: Vec::new(),
        })
    } else if milestone_only {
        let mid = milestone_id.as_deref().context("No active milestone")?;
        Ok(ArtifactIndex {
            global: Vec::new(),
            milestone: ArtifactIndex::load_milestone(root, mid)?,
        })
    } else {
        ArtifactIndex::load(root, milestone_id.as_deref())
    }
}

/// Returns a single artifact as data (no stdout).
pub fn get_artifact_show(
    root: &Path,
    name: &str,
    global_only: bool,
    milestone_only: bool,
) -> Result<ArtifactFull> {
    let milestone_id = current_milestone_id(root);

    if global_only {
        find_artifact(root, name, true, None)
    } else if milestone_only {
        let mid = milestone_id.as_deref().context("No active milestone")?;
        find_artifact(root, name, false, Some(mid))
    } else {
        let mid = milestone_id.as_deref();
        if let Some(mid) = mid {
            find_artifact(root, name, false, Some(mid))
                .or_else(|_| find_artifact(root, name, true, None))
        } else {
            find_artifact(root, name, true, None)
        }
    }
}

fn find_artifact(
    root: &Path,
    name: &str,
    global: bool,
    milestone_id: Option<&str>,
) -> Result<ArtifactFull> {
    let dir = if global {
        root.join("human/artifacts")
    } else {
        root.join("human/milestones")
            .join(milestone_id.unwrap())
            .join("artifacts")
    };
    let path = dir.join(format!("{name}.md"));
    anyhow::ensure!(path.exists(), "Artifact '{}' not found", name);
    ArtifactFull::load(&path)
}

fn current_milestone_id(root: &Path) -> Option<String> {
    let path = root.join("milestones.yaml");
    if !path.exists() {
        return None;
    }
    MilestoneMap::load(&path)
        .ok()
        .and_then(|m| m.current.map(|c| c.id))
}

// Display for ArtifactKind
impl std::fmt::Display for crate::model::artifact::ArtifactKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Context => write!(f, "context"),
            Self::Stack => write!(f, "stack"),
            Self::Constraints => write!(f, "constraints"),
            Self::Decision => write!(f, "decision"),
            Self::Feature => write!(f, "feature"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}
