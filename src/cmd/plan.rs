use std::path::Path;

use anyhow::Result;
use colored::Colorize;

use super::style;
use crate::model::milestone::{MilestoneMap, StageEntry, StageStatus};
use crate::model::stage::{StagePlan, StageTask};
use crate::model::task::TaskStatus;

/// Resolve task status: prefer milestones.yaml tracker, fallback to stage_N.md `status:` field.
pub fn resolve_task_status(task: &StageTask, stage_entry: &StageEntry) -> Option<TaskStatus> {
    // First: check milestones.yaml tracker
    if let Some(tracker) = stage_entry.tasks.iter().find(|t| t.id == task.id) {
        return Some(tracker.status.clone());
    }
    // Fallback: parse status from stage_N.md
    task.status.as_ref().and_then(|s| match s.as_str() {
        "done" | "completed" => Some(TaskStatus::Done),
        "in_progress" => Some(TaskStatus::InProgress),
        "blocked" => Some(TaskStatus::Blocked),
        "pending" => Some(TaskStatus::Pending),
        _ => None,
    })
}

#[derive(serde::Serialize)]
pub struct PlanData {
    pub milestone_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_content: Option<String>,
    pub stages: Vec<PlanStageData>,
}

#[derive(serde::Serialize)]
pub struct PlanStageData {
    pub id: u32,
    pub scope: String,
    pub status: String,
    pub tasks: Vec<PlanTaskData>,
    pub remediation: Vec<PlanTaskData>,
}

#[derive(serde::Serialize)]
pub struct PlanTaskData {
    pub id: String,
    pub name: String,
    pub depends_on: Vec<String>,
    pub contracts: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

pub fn get_plan(project_root: &Path) -> Result<Option<PlanData>> {
    let milestones = MilestoneMap::load(&project_root.join("milestones.yaml"))?;
    let current = match &milestones.current {
        Some(c) => c,
        None => return Ok(None),
    };

    let milestone_dir = project_root.join("human/milestones").join(&current.id);
    let plan_path = milestone_dir.join("plan.md");
    let plan_content = if plan_path.exists() {
        Some(std::fs::read_to_string(&plan_path)?)
    } else {
        None
    };

    let mut stages = Vec::new();
    for se in &current.stages {
        let stage_file = milestone_dir.join(format!("stage_{}.md", se.id));
        let (tasks, remediation) = if let Ok(sp) = StagePlan::load(&stage_file) {
            let tasks = sp
                .tasks
                .iter()
                .map(|t| {
                    let status = resolve_task_status(t, se).map(|s| s.to_string());
                    PlanTaskData {
                        id: t.id.clone(),
                        name: t.name.clone(),
                        depends_on: t.depends_on.clone(),
                        contracts: t.contracts.clone(),
                        status,
                    }
                })
                .collect();
            let rem = sp
                .remediation
                .iter()
                .map(|t| {
                    let status = resolve_task_status(t, se).map(|s| s.to_string());
                    PlanTaskData {
                        id: t.id.clone(),
                        name: t.name.clone(),
                        depends_on: t.depends_on.clone(),
                        contracts: t.contracts.clone(),
                        status,
                    }
                })
                .collect();
            (tasks, rem)
        } else {
            (Vec::new(), Vec::new())
        };
        stages.push(PlanStageData {
            id: se.id,
            scope: se.scope.clone(),
            status: se.status.to_string(),
            tasks,
            remediation,
        });
    }

    Ok(Some(PlanData {
        milestone_id: current.id.clone(),
        plan_content,
        stages,
    }))
}

pub fn run(project_root: &Path, visual: bool, json: bool) -> Result<()> {
    if json {
        let data = get_plan(project_root)?;
        println!("{}", serde_json::to_string_pretty(&data)?);
        return Ok(());
    }

    let milestones = MilestoneMap::load(&project_root.join("milestones.yaml"))?;

    style::header("plan");

    let current = match &milestones.current {
        Some(c) => c,
        None => {
            style::hint("No active milestone. Run `hlv milestone new <name>` to start.");
            return Ok(());
        }
    };

    // Read plan.md for the milestone
    let milestone_dir = project_root.join("human/milestones").join(&current.id);
    let plan_path = milestone_dir.join("plan.md");

    if plan_path.exists() {
        let plan_content = std::fs::read_to_string(&plan_path)?;
        println!();
        // Print plan.md header lines (up to ## Stages table)
        for line in plan_content.lines() {
            println!("  {}", line);
        }
        println!();
    }

    if current.stages.is_empty() {
        style::hint("No stages yet. Run /generate to create them.");
        return Ok(());
    }

    if visual {
        print_visual_milestone(current, &milestone_dir);
    } else {
        print_table_milestone(current, &milestone_dir);
    }

    Ok(())
}

fn print_table_milestone(
    current: &crate::model::milestone::MilestoneCurrent,
    milestone_dir: &Path,
) {
    let validated = current
        .stages
        .iter()
        .filter(|s| s.status == StageStatus::Validated)
        .count();
    println!(
        "  Stages: {} | Validated: {}/{}\n",
        current.stages.len(),
        validated,
        current.stages.len(),
    );

    for stage_entry in &current.stages {
        let icon = match stage_entry.status {
            StageStatus::Validated => "✓".green(),
            StageStatus::Verified => "✓".green(),
            StageStatus::Implementing | StageStatus::Validating => "▸".yellow(),
            StageStatus::Implemented => "●".cyan(),
            StageStatus::Pending => "○".dimmed(),
        };
        let active = if current.stage == Some(stage_entry.id) {
            " ◀"
        } else {
            ""
        };
        println!(
            "  {} Stage {}: {} [{}]{}",
            icon,
            stage_entry.id,
            stage_entry.scope.bold(),
            super::status::colorize_status(&stage_entry.status.to_string()),
            active.cyan()
        );

        // Try to load stage_N.md for task details
        let stage_file = milestone_dir.join(format!("stage_{}.md", stage_entry.id));
        if let Ok(stage) = StagePlan::load(&stage_file) {
            for task in &stage.tasks {
                let status = resolve_task_status(task, stage_entry);
                let icon = match status.as_ref() {
                    Some(TaskStatus::Done) => "✓".green(),
                    Some(TaskStatus::InProgress) => "▸".yellow(),
                    Some(TaskStatus::Blocked) => "✗".red(),
                    _ => "○".dimmed(),
                };
                let deps = if task.depends_on.is_empty() {
                    String::new()
                } else {
                    format!(" (after {})", task.depends_on.join(", "))
                        .dimmed()
                        .to_string()
                };
                println!("    {} {} {}{}", icon, task.id, task.name, deps);
            }
            if !stage.remediation.is_empty() {
                for fix in &stage.remediation {
                    println!("    {} {} {}", "!".red(), fix.id, fix.name);
                }
            }
        }
        println!();
    }

    println!(
        "  Stage: {} validated  {} active  {} implemented  {} pending",
        "✓".green(),
        "▸".yellow(),
        "●".cyan(),
        "○".dimmed()
    );
    println!(
        "  Task:  {} done  {} in progress  {} blocked  {} pending",
        "✓".green(),
        "▸".yellow(),
        "✗".red(),
        "○".dimmed()
    );
}

fn print_visual_milestone(
    current: &crate::model::milestone::MilestoneCurrent,
    milestone_dir: &Path,
) {
    println!();
    for (i, stage_entry) in current.stages.iter().enumerate() {
        let header = format!("Stage {} — {}", stage_entry.id, stage_entry.scope);
        let status_str = format!("[{}]", stage_entry.status);
        let width = 44;

        println!("  ┌{}┐", "─".repeat(width));
        println!("  │ {:<w$}│", header, w = width - 1);
        println!("  │ {:<w$}│", status_str, w = width - 1);
        println!("  ├{}┤", "─".repeat(width));

        let stage_file = milestone_dir.join(format!("stage_{}.md", stage_entry.id));
        if let Ok(stage) = StagePlan::load(&stage_file) {
            for task in &stage.tasks {
                let status = resolve_task_status(task, stage_entry);
                let task_icon = match status.as_ref() {
                    Some(TaskStatus::Done) => "✓",
                    Some(TaskStatus::InProgress) => "▸",
                    Some(TaskStatus::Blocked) => "✗",
                    _ => "○",
                };
                let line = format!("{} {} {}", task_icon, task.id, task.name);
                let display = if line.len() > width - 2 {
                    format!("{}…", &line[..width - 3])
                } else {
                    line
                };
                println!("  │ {:<w$}│", display, w = width - 1);
            }
        }
        println!("  └{}┘", "─".repeat(width));

        if i < current.stages.len() - 1 {
            println!("  {:>w$}", "│", w = width / 2 + 2);
            println!("  {:>w$}", "▼", w = width / 2 + 2);
        }
    }
    println!();
}
