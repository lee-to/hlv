use std::path::Path;

use anyhow::Result;
use colored::Colorize;

use super::style;
use crate::model::milestone::{GateRunStatus, MilestoneMap, StageStatus};
use crate::model::policy::GatesPolicy;
use crate::model::project::ProjectMap;
use crate::model::task::TaskStatus;

/// Structured status data for JSON output
#[derive(serde::Serialize)]
pub struct StatusData {
    pub project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestone: Option<MilestoneStatusData>,
    pub history_count: usize,
}

#[derive(serde::Serialize)]
pub struct MilestoneStatusData {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_stage: Option<u32>,
    pub stages: Vec<StageStatusData>,
    pub contracts: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct StageStatusData {
    pub id: u32,
    pub scope: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    pub task_count: usize,
    pub tasks_done: usize,
    pub tasks_in_progress: usize,
    pub tasks_blocked: usize,
}

pub fn get_status(project_root: &Path) -> Result<StatusData> {
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;
    let milestones = MilestoneMap::load(&project_root.join("milestones.yaml"))?;

    let milestone = milestones.current.as_ref().map(|current| {
        let contracts = collect_contracts(project_root, &current.id);
        MilestoneStatusData {
            id: current.id.clone(),
            branch: current.branch.clone(),
            active_stage: current.stage,
            stages: current
                .stages
                .iter()
                .map(|s| {
                    let task_count = s.tasks.len();
                    let tasks_done = s
                        .tasks
                        .iter()
                        .filter(|t| t.status == TaskStatus::Done)
                        .count();
                    let tasks_in_progress = s
                        .tasks
                        .iter()
                        .filter(|t| t.status == TaskStatus::InProgress)
                        .count();
                    let tasks_blocked = s
                        .tasks
                        .iter()
                        .filter(|t| t.status == TaskStatus::Blocked)
                        .count();
                    StageStatusData {
                        id: s.id,
                        scope: s.scope.clone(),
                        status: s.status.to_string(),
                        commit: s.commit.clone(),
                        task_count,
                        tasks_done,
                        tasks_in_progress,
                        tasks_blocked,
                    }
                })
                .collect(),
            contracts,
        }
    });

    Ok(StatusData {
        project: project.project,
        milestone,
        history_count: milestones.history.len(),
    })
}

fn collect_contracts(root: &Path, milestone_id: &str) -> Vec<String> {
    let contracts_dir = root
        .join("human/milestones")
        .join(milestone_id)
        .join("contracts");
    if !contracts_dir.is_dir() {
        return Vec::new();
    }
    std::fs::read_dir(&contracts_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.strip_suffix(".md").map(|n| n.to_string())
        })
        .collect()
}

pub fn run(project_root: &Path, json: bool) -> Result<()> {
    if json {
        let data = get_status(project_root)?;
        println!("{}", serde_json::to_string_pretty(&data)?);
        return Ok(());
    }

    let project = ProjectMap::load(&project_root.join("project.yaml"))?;
    let milestones = MilestoneMap::load(&project_root.join("milestones.yaml"))?;

    style::header("status");

    style::detail("Project", &project.project);

    match &milestones.current {
        Some(current) => {
            style::detail("Milestone", &current.id);
            if let Some(branch) = &current.branch {
                style::detail("Branch", branch);
            }

            if current.stages.is_empty() {
                style::detail("Stage", "none (run /generate)");
            } else {
                let validated = current
                    .stages
                    .iter()
                    .filter(|s| s.status == StageStatus::Validated)
                    .count();
                let total = current.stages.len();
                style::detail(
                    "Progress",
                    &format!("{}/{} stages validated", validated, total),
                );

                style::section("Stages");
                for s in &current.stages {
                    let icon = match s.status {
                        StageStatus::Validated => "✓".green(),
                        StageStatus::Verified => "✓".green(),
                        StageStatus::Implementing | StageStatus::Validating => "▸".yellow(),
                        StageStatus::Implemented => "●".cyan(),
                        StageStatus::Pending => "○".dimmed(),
                    };
                    let active_marker = if current.stage == Some(s.id) {
                        " ◀".cyan().to_string()
                    } else {
                        String::new()
                    };
                    let commit_note = s
                        .commit
                        .as_ref()
                        .map(|c| format!(" ({})", &c[..7.min(c.len())]))
                        .unwrap_or_default();
                    println!(
                        "    {} Stage {}: {} [{}]{}{}",
                        icon,
                        s.id,
                        s.scope,
                        colorize_status(&s.status.to_string()),
                        commit_note.dimmed(),
                        active_marker
                    );
                    if !s.tasks.is_empty() {
                        let total = s.tasks.len();
                        let done = s
                            .tasks
                            .iter()
                            .filter(|t| t.status == TaskStatus::Done)
                            .count();
                        let in_progress = s
                            .tasks
                            .iter()
                            .filter(|t| t.status == TaskStatus::InProgress)
                            .count();
                        let blocked = s
                            .tasks
                            .iter()
                            .filter(|t| t.status == TaskStatus::Blocked)
                            .count();
                        let mut parts = vec![format!("{}/{} done", done, total)];
                        if in_progress > 0 {
                            parts.push(format!("{} in progress", in_progress));
                        }
                        if blocked > 0 {
                            parts.push(format!("{} blocked", blocked));
                        }
                        println!("      Tasks: {}", parts.join(", "));
                    }
                }
            }

            // Show milestone contracts if dir exists
            let contracts_dir = project_root
                .join("human/milestones")
                .join(&current.id)
                .join("contracts");
            if contracts_dir.is_dir() {
                let contracts: Vec<String> = std::fs::read_dir(&contracts_dir)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        name.strip_suffix(".md").map(|n| n.to_string())
                    })
                    .collect();
                if !contracts.is_empty() {
                    style::section("Contracts");
                    for c in &contracts {
                        println!("    {} {}", "●".cyan(), c);
                    }
                }
            }
        }
        None => {
            style::detail("Milestone", "none");
            style::hint("Run `hlv milestone new <name>` to start a milestone.");
        }
    }

    // History summary
    if !milestones.history.is_empty() {
        let merged = milestones
            .history
            .iter()
            .filter(|h| h.status == crate::model::milestone::MilestoneStatus::Merged)
            .count();
        style::detail("History", &format!("{} milestone(s) completed", merged));
    }

    // Gates
    let policy_path = project_root.join(&project.paths.validation.gates_policy);
    if let Ok(policy) = GatesPolicy::load(&policy_path) {
        let gate_results = milestones
            .current
            .as_ref()
            .map(|c| &c.gate_results[..])
            .unwrap_or(&[]);

        style::section("Gates");
        for gate in &policy.gates {
            let result = gate_results.iter().find(|r| r.id == gate.id);
            let (icon, label) = match result {
                Some(r) => match r.status {
                    GateRunStatus::Passed => ("✓".green(), "passed".green()),
                    GateRunStatus::Failed => ("✗".red(), "failed".red()),
                    GateRunStatus::Skipped => ("○".dimmed(), "skipped".dimmed()),
                },
                None => ("○".dimmed(), "not_run".dimmed()),
            };
            println!("    {} {} ({})", icon, gate.id, label);
        }
    }

    println!();
    Ok(())
}

pub(crate) fn colorize_status(s: &str) -> colored::ColoredString {
    match s {
        "validated" | "implemented" | "verified" | "passed" | "completed" => s.green(),
        "implementing" | "validating" | "in_progress" => s.yellow(),
        "draft" | "pending" | "not_run" => s.dimmed(),
        "failed" => s.red(),
        _ => s.normal(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use colored::Colorize;

    #[test]
    fn colorize_green_variants() {
        for s in &[
            "validated",
            "implemented",
            "verified",
            "passed",
            "completed",
        ] {
            let c = colorize_status(s);
            assert_eq!(c, s.green(), "expected green for '{}'", s);
        }
    }

    #[test]
    fn colorize_red() {
        assert_eq!(colorize_status("failed"), "failed".red());
    }

    #[test]
    fn colorize_yellow_variants() {
        for s in &["implementing", "validating", "in_progress"] {
            let c = colorize_status(s);
            assert_eq!(c, s.yellow(), "expected yellow for '{}'", s);
        }
    }

    #[test]
    fn colorize_dimmed_variants() {
        for s in &["draft", "pending", "not_run"] {
            let c = colorize_status(s);
            assert_eq!(c, s.dimmed(), "expected dimmed for '{}'", s);
        }
    }

    #[test]
    fn colorize_default() {
        assert_eq!(colorize_status("unknown_xyz"), "unknown_xyz".normal());
    }
}
