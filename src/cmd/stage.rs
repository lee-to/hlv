use anyhow::{Context, Result};
use std::path::Path;

use super::style;
use crate::model::milestone::{MilestoneMap, StageStatus};

/// `hlv stage label <stage-id> add|remove <label>`
pub fn run_label(root: &Path, stage_id: u32, action: &str, label: &str) -> Result<()> {
    let mut map = load(root)?;
    let stage = find_stage_mut(&mut map, stage_id)?;

    match action {
        "add" => {
            if !stage.labels.contains(&label.to_string()) {
                stage.labels.push(label.to_string());
            }
        }
        "remove" => {
            stage.labels.retain(|l| l != label);
        }
        _ => anyhow::bail!("Unknown label action: {}. Use 'add' or 'remove'.", action),
    }

    save(root, &map)?;
    style::ok(&format!("Stage {} label {} {}", stage_id, action, label));
    Ok(())
}

/// `hlv stage meta <stage-id> set|delete <key> [<value>]`
pub fn run_meta(
    root: &Path,
    stage_id: u32,
    action: &str,
    key: &str,
    value: Option<&str>,
) -> Result<()> {
    let mut map = load(root)?;
    let stage = find_stage_mut(&mut map, stage_id)?;

    match action {
        "set" => {
            let val = value.context("Value required for 'set'")?;
            stage.meta.insert(key.to_string(), val.to_string());
        }
        "delete" => {
            stage.meta.remove(key);
        }
        _ => anyhow::bail!("Unknown meta action: {}. Use 'set' or 'delete'.", action),
    }

    save(root, &map)?;
    style::ok(&format!("Stage {} meta {} {}", stage_id, action, key));
    Ok(())
}

/// `hlv milestone label add|remove <label>`
pub fn run_milestone_label(root: &Path, action: &str, label: &str) -> Result<()> {
    let mut map = load(root)?;
    let current = map.current.as_mut().context("No active milestone")?;

    match action {
        "add" => {
            if !current.labels.contains(&label.to_string()) {
                current.labels.push(label.to_string());
            }
        }
        "remove" => {
            current.labels.retain(|l| l != label);
        }
        _ => anyhow::bail!("Unknown label action: {}. Use 'add' or 'remove'.", action),
    }

    save(root, &map)?;
    style::ok(&format!("Milestone label {} {}", action, label));
    Ok(())
}

/// `hlv milestone meta set|delete <key> [<value>]`
pub fn run_milestone_meta(root: &Path, action: &str, key: &str, value: Option<&str>) -> Result<()> {
    let mut map = load(root)?;
    let current = map.current.as_mut().context("No active milestone")?;

    match action {
        "set" => {
            let val = value.context("Value required for 'set'")?;
            current.meta.insert(key.to_string(), val.to_string());
        }
        "delete" => {
            current.meta.remove(key);
        }
        _ => anyhow::bail!("Unknown meta action: {}. Use 'set' or 'delete'.", action),
    }

    save(root, &map)?;
    style::ok(&format!("Milestone meta {} {}", action, key));
    Ok(())
}

/// `hlv stage reopen <stage-id>`
///
/// Reverts a stage to its previous active status:
///   implemented → implementing
///   validated   → validating
///   validating  → implementing
///
/// Use when manual review finds issues after implementation or validation.
pub fn run_reopen(root: &Path, stage_id: u32) -> Result<()> {
    let mut map = load(root)?;
    let stage = find_stage_mut(&mut map, stage_id)?;

    let new_status = match stage.status {
        StageStatus::Implemented => StageStatus::Implementing,
        StageStatus::Validated => StageStatus::Validating,
        StageStatus::Validating => StageStatus::Implementing,
        ref s => anyhow::bail!(
            "Cannot reopen stage {} — status is {} (must be implemented, validating, or validated)",
            stage_id,
            s
        ),
    };

    let old = stage.status.to_string();
    stage.status = new_status.clone();

    // Point the active stage cursor to this stage
    map.current.as_mut().unwrap().stage = Some(stage_id);

    save(root, &map)?;
    style::ok(&format!(
        "Stage {} reopened: {} → {}",
        stage_id, old, new_status
    ));
    Ok(())
}

// ── Helpers ──────────────────────────────────

fn load(root: &Path) -> Result<MilestoneMap> {
    let path = root.join("milestones.yaml");
    anyhow::ensure!(path.exists(), "milestones.yaml not found");
    MilestoneMap::load(&path)
}

fn save(root: &Path, map: &MilestoneMap) -> Result<()> {
    map.save(&root.join("milestones.yaml"))
}

fn find_stage_mut(
    map: &mut MilestoneMap,
    stage_id: u32,
) -> Result<&mut crate::model::milestone::StageEntry> {
    let current = map.current.as_mut().context("No active milestone")?;
    current
        .stages
        .iter_mut()
        .find(|s| s.id == stage_id)
        .context(format!("Stage {} not found", stage_id))
}
