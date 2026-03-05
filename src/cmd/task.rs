use anyhow::{Context, Result};
use colored::Colorize;
use std::path::Path;

use super::style;
use crate::model::milestone::{MilestoneMap, StageEntry, StageStatus};
use crate::model::stage::StagePlan;
use crate::model::task::{TaskStatus, TaskTracker};

/// `hlv task list [--stage N] [--status S] [--label L] [--json]`
pub fn run_list(
    root: &Path,
    stage_filter: Option<u32>,
    status_filter: Option<&str>,
    label_filter: Option<&str>,
    json: bool,
) -> Result<()> {
    let map = load(root)?;
    let current = map.current.as_ref().context("No active milestone")?;

    let status_enum = status_filter.map(parse_status).transpose()?;

    let mut all_tasks: Vec<TaskView> = Vec::new();

    for stage in &current.stages {
        if let Some(sf) = stage_filter {
            if stage.id != sf {
                continue;
            }
        }
        for task in &stage.tasks {
            if let Some(ref s) = status_enum {
                if &task.status != s {
                    continue;
                }
            }
            if let Some(label) = label_filter {
                if !task.labels.contains(&label.to_string()) {
                    continue;
                }
            }
            all_tasks.push(TaskView {
                stage_id: stage.id,
                id: task.id.clone(),
                status: task.status.to_string(),
                started_at: task.started_at.clone(),
                completed_at: task.completed_at.clone(),
                block_reason: task.block_reason.clone(),
                labels: task.labels.clone(),
                meta: task.meta.clone(),
            });
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&all_tasks)?);
    } else {
        if all_tasks.is_empty() {
            style::hint("No tasks found. Run `hlv task sync` to populate from stage plans.");
            return Ok(());
        }
        for t in &all_tasks {
            let icon = match t.status.as_str() {
                "done" => "✓".green(),
                "in_progress" => "▸".yellow(),
                "blocked" => "✗".red(),
                _ => "○".dimmed(),
            };
            let labels_str = if t.labels.is_empty() {
                String::new()
            } else {
                format!(" [{}]", t.labels.join(", "))
            };
            println!(
                "  {} Stage {} / {} [{}]{}",
                icon, t.stage_id, t.id, t.status, labels_str
            );
        }
    }
    Ok(())
}

/// Pure data function: return filtered task list for MCP/JSON consumers.
pub fn get_task_list(
    root: &Path,
    stage_filter: Option<u32>,
    status_filter: Option<&str>,
    label_filter: Option<&str>,
) -> Result<Vec<TaskView>> {
    let map = load(root)?;
    let current = map.current.as_ref().context("No active milestone")?;
    let status_enum = status_filter.map(parse_status).transpose()?;

    let mut all_tasks: Vec<TaskView> = Vec::new();
    for stage in &current.stages {
        if let Some(sf) = stage_filter {
            if stage.id != sf {
                continue;
            }
        }
        for task in &stage.tasks {
            if let Some(ref s) = status_enum {
                if &task.status != s {
                    continue;
                }
            }
            if let Some(label) = label_filter {
                if !task.labels.contains(&label.to_string()) {
                    continue;
                }
            }
            all_tasks.push(TaskView {
                stage_id: stage.id,
                id: task.id.clone(),
                status: task.status.to_string(),
                started_at: task.started_at.clone(),
                completed_at: task.completed_at.clone(),
                block_reason: task.block_reason.clone(),
                labels: task.labels.clone(),
                meta: task.meta.clone(),
            });
        }
    }
    Ok(all_tasks)
}

/// `hlv task start <task-id>`
pub fn run_start(root: &Path, task_id: &str) -> Result<()> {
    let (mut map, stage_idx, task_idx) = find_task_mut(root, task_id)?;
    let milestone_id = map.current.as_ref().unwrap().id.clone();

    // Check dependencies (needs read access to all stages)
    check_dependencies(
        root,
        &milestone_id,
        map.current.as_ref().unwrap(),
        stage_idx,
        task_id,
    )?;

    let stage = &mut map.current.as_mut().unwrap().stages[stage_idx];

    let now = now_iso();
    stage.tasks[task_idx].start(&now)?;

    // Auto-transition stage to Implementing if Pending
    if stage.status == StageStatus::Pending || stage.status == StageStatus::Verified {
        stage.status = StageStatus::Implementing;
    }

    save(root, &map)?;
    style::ok(&format!("Task {} started", task_id.bold()));
    Ok(())
}

/// `hlv task done <task-id>`
pub fn run_done(root: &Path, task_id: &str) -> Result<()> {
    let (mut map, stage_idx, task_idx) = find_task_mut(root, task_id)?;
    let stage = &mut map.current.as_mut().unwrap().stages[stage_idx];

    let now = now_iso();
    stage.tasks[task_idx].done(&now)?;

    // Check if all tasks done
    let all_done = stage.tasks.iter().all(|t| t.status == TaskStatus::Done);
    save(root, &map)?;

    style::ok(&format!("Task {} done", task_id.bold()));
    if all_done {
        style::hint("All tasks in this stage are done. Consider `hlv milestone status` to review.");
    }
    Ok(())
}

/// `hlv task block <task-id> --reason "..."`
pub fn run_block(root: &Path, task_id: &str, reason: &str) -> Result<()> {
    let (mut map, stage_idx, task_idx) = find_task_mut(root, task_id)?;
    let stage = &mut map.current.as_mut().unwrap().stages[stage_idx];

    stage.tasks[task_idx].block(reason)?;
    save(root, &map)?;
    style::ok(&format!("Task {} blocked: {}", task_id.bold(), reason));
    Ok(())
}

/// `hlv task unblock <task-id>`
pub fn run_unblock(root: &Path, task_id: &str) -> Result<()> {
    let (mut map, stage_idx, task_idx) = find_task_mut(root, task_id)?;
    let stage = &mut map.current.as_mut().unwrap().stages[stage_idx];

    stage.tasks[task_idx].unblock()?;
    save(root, &map)?;
    style::ok(&format!("Task {} unblocked", task_id.bold()));
    Ok(())
}

/// `hlv task status [--json]`
pub fn run_status(root: &Path, json: bool) -> Result<()> {
    let map = load(root)?;
    let current = map.current.as_ref().context("No active milestone")?;

    let mut summary = TaskSummary {
        milestone: current.id.clone(),
        stages: Vec::new(),
    };

    for stage in &current.stages {
        let pending = stage
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Pending)
            .count();
        let in_progress = stage
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::InProgress)
            .count();
        let done = stage
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Done)
            .count();
        let blocked = stage
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Blocked)
            .count();

        summary.stages.push(StageSummary {
            stage_id: stage.id,
            scope: stage.scope.clone(),
            total: stage.tasks.len(),
            pending,
            in_progress,
            done,
            blocked,
        });
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        style::header("task status");
        println!("  {} {}", "Milestone:".bold(), summary.milestone);
        for s in &summary.stages {
            println!(
                "  Stage {}: {} — {}/{} done, {} in progress, {} blocked",
                s.stage_id, s.scope, s.done, s.total, s.in_progress, s.blocked
            );
        }
    }
    Ok(())
}

/// `hlv task sync`
pub fn run_sync(root: &Path, force: bool) -> Result<()> {
    let mut map = load(root)?;
    let current = map.current.as_mut().context("No active milestone")?;
    let milestone_dir = root.join("human/milestones").join(&current.id);

    // Discover stage_N.md files and create missing StageEntry records
    if milestone_dir.exists() {
        let mut discovered: Vec<u32> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&milestone_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if let Some(rest) = name.strip_prefix("stage_") {
                    if let Some(num_str) = rest.strip_suffix(".md") {
                        if let Ok(id) = num_str.parse::<u32>() {
                            discovered.push(id);
                        }
                    }
                }
            }
        }
        discovered.sort();
        for stage_id in discovered {
            if !current.stages.iter().any(|s| s.id == stage_id) {
                // Parse stage plan to get scope name
                let stage_path = milestone_dir.join(format!("stage_{}.md", stage_id));
                let scope = StagePlan::load(&stage_path)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|_| format!("Stage {}", stage_id));
                current.stages.push(StageEntry {
                    id: stage_id,
                    scope,
                    status: StageStatus::Pending,
                    commit: None,
                    tasks: Vec::new(),
                    labels: Vec::new(),
                    meta: std::collections::HashMap::new(),
                });
            }
        }
        // Keep stages sorted by id
        current.stages.sort_by_key(|s| s.id);
    }

    let mut added = 0u32;
    let mut removed = 0u32;
    let mut conflicts = Vec::new();

    for stage in &mut current.stages {
        let stage_path = milestone_dir.join(format!("stage_{}.md", stage.id));
        if !stage_path.exists() {
            continue;
        }
        let plan = StagePlan::load(&stage_path)?;

        // Collect all task IDs from stage_N.md
        let mut plan_ids: Vec<String> = plan.tasks.iter().map(|t| t.id.clone()).collect();
        plan_ids.extend(plan.remediation.iter().map(|t| t.id.clone()));

        // Add new tasks (in plan, not in tracker)
        let existing_ids: Vec<String> = stage.tasks.iter().map(|t| t.id.clone()).collect();
        for id in &plan_ids {
            if !existing_ids.contains(id) {
                stage.tasks.push(TaskTracker::new(id.clone()));
                added += 1;
            }
        }

        // Remove tasks (in tracker, not in plan)
        let mut to_remove = Vec::new();
        for (i, tracker) in stage.tasks.iter().enumerate() {
            if !plan_ids.contains(&tracker.id) {
                if tracker.status == TaskStatus::Pending || force {
                    to_remove.push(i);
                    removed += 1;
                } else {
                    conflicts.push(format!(
                        "{} (stage {}, status: {})",
                        tracker.id, stage.id, tracker.status
                    ));
                }
            }
        }
        // Remove in reverse order to preserve indices
        for i in to_remove.into_iter().rev() {
            stage.tasks.remove(i);
        }
    }

    if !conflicts.is_empty() && !force {
        anyhow::bail!(
            "Cannot remove active tasks:\n  {}\nUse `hlv task sync --force` to remove anyway.",
            conflicts.join("\n  ")
        );
    }

    save(root, &map)?;
    style::ok(&format!(
        "Sync complete: {} added, {} removed",
        added, removed
    ));
    Ok(())
}

/// `hlv task label <task-id> add|remove <label>`
pub fn run_label(root: &Path, task_id: &str, action: &str, label: &str) -> Result<()> {
    let (mut map, stage_idx, task_idx) = find_task_mut(root, task_id)?;
    let task = &mut map.current.as_mut().unwrap().stages[stage_idx].tasks[task_idx];

    match action {
        "add" => {
            if !task.labels.contains(&label.to_string()) {
                task.labels.push(label.to_string());
            }
        }
        "remove" => {
            task.labels.retain(|l| l != label);
        }
        _ => anyhow::bail!("Unknown label action: {}. Use 'add' or 'remove'.", action),
    }

    save(root, &map)?;
    style::ok(&format!("Task {} label {} {}", task_id, action, label));
    Ok(())
}

/// `hlv task meta <task-id> set|delete <key> [<value>]`
pub fn run_meta(
    root: &Path,
    task_id: &str,
    action: &str,
    key: &str,
    value: Option<&str>,
) -> Result<()> {
    let (mut map, stage_idx, task_idx) = find_task_mut(root, task_id)?;
    let task = &mut map.current.as_mut().unwrap().stages[stage_idx].tasks[task_idx];

    match action {
        "set" => {
            let val = value.context("Value required for 'set'")?;
            task.meta.insert(key.to_string(), val.to_string());
        }
        "delete" => {
            task.meta.remove(key);
        }
        _ => anyhow::bail!("Unknown meta action: {}. Use 'set' or 'delete'.", action),
    }

    save(root, &map)?;
    style::ok(&format!("Task {} meta {} {}", task_id, action, key));
    Ok(())
}

/// `hlv task add --stage <N> <task-id> <name>`
///
/// Adds a new task to both stage_N.md and milestones.yaml tracker.
/// If the stage is `implemented` or `validated`, it auto-reopens to `implementing`.
pub fn run_add(root: &Path, stage_id: u32, task_id: &str, name: &str) -> Result<()> {
    let mut map = load(root)?;
    let current = map.current.as_mut().context("No active milestone")?;
    let milestone_id = current.id.clone();

    // Validate task ID format
    if !task_id.starts_with("TASK-") && !task_id.starts_with("FIX-") {
        anyhow::bail!("Task ID must start with TASK- or FIX- (got: {})", task_id);
    }

    // Check for duplicate across all stages
    for stage in &current.stages {
        if stage.tasks.iter().any(|t| t.id == task_id) {
            anyhow::bail!("Task {} already exists in stage {}", task_id, stage.id);
        }
    }

    let stage = current
        .stages
        .iter_mut()
        .find(|s| s.id == stage_id)
        .context(format!("Stage {} not found", stage_id))?;

    // Auto-reopen if needed
    let reopened = match stage.status {
        StageStatus::Implemented | StageStatus::Validated | StageStatus::Validating => {
            let old = stage.status.to_string();
            stage.status = StageStatus::Implementing;
            current.stage = Some(stage_id);
            Some(old)
        }
        _ => None,
    };

    // Add tracker to milestones.yaml
    stage.tasks.push(TaskTracker::new(task_id.to_string()));

    // Append task to stage_N.md
    let milestone_dir = root.join("human/milestones").join(&milestone_id);
    let stage_path = milestone_dir.join(format!("stage_{}.md", stage_id));
    anyhow::ensure!(stage_path.exists(), "stage_{}.md not found", stage_id);

    let content = std::fs::read_to_string(&stage_path)?;
    let task_entry = format!("\n{} {}\n  contracts: []\n", task_id, name);

    // Insert before ## Remediation if present, else append to Tasks section
    let new_content = if let Some(pos) = content.find("## Remediation") {
        let (before, after) = content.split_at(pos);
        format!("{}{}{}", before, task_entry, after)
    } else {
        format!("{}{}", content.trim_end(), task_entry)
    };

    std::fs::write(&stage_path, new_content)?;
    save(root, &map)?;

    style::ok(&format!("Task {} added to stage {}", task_id, stage_id));
    if let Some(old) = reopened {
        style::hint(&format!(
            "Stage {} reopened: {} → implementing",
            stage_id, old
        ));
    }
    Ok(())
}

// ── Internal helpers ──────────────────────────

fn load(root: &Path) -> Result<MilestoneMap> {
    let path = root.join("milestones.yaml");
    anyhow::ensure!(path.exists(), "milestones.yaml not found");
    MilestoneMap::load(&path)
}

fn save(root: &Path, map: &MilestoneMap) -> Result<()> {
    map.save(&root.join("milestones.yaml"))
}

/// Find a task across all stages, return (map, stage_index, task_index)
fn find_task_mut(root: &Path, task_id: &str) -> Result<(MilestoneMap, usize, usize)> {
    let map = load(root)?;
    let current = map.current.as_ref().context("No active milestone")?;

    for (si, stage) in current.stages.iter().enumerate() {
        for (ti, task) in stage.tasks.iter().enumerate() {
            if task.id == task_id {
                return Ok((map, si, ti));
            }
        }
    }
    anyhow::bail!("Task {} not found in any stage", task_id)
}

/// Check that all dependencies of task_id are Done (searches across all stages)
fn check_dependencies(
    root: &Path,
    milestone_id: &str,
    current: &crate::model::milestone::MilestoneCurrent,
    stage_idx: usize,
    task_id: &str,
) -> Result<()> {
    let milestone_dir = root.join("human/milestones").join(milestone_id);
    let stage = &current.stages[stage_idx];
    let stage_path = milestone_dir.join(format!("stage_{}.md", stage.id));
    if !stage_path.exists() {
        return Ok(()); // No plan file — skip dep check
    }
    let plan = StagePlan::load(&stage_path)?;
    let all_tasks: Vec<_> = plan.tasks.iter().chain(plan.remediation.iter()).collect();

    if let Some(task_plan) = all_tasks.iter().find(|t| t.id == task_id) {
        for dep_id in &task_plan.depends_on {
            // Search across ALL stages for the dependency
            let dep_done = current
                .stages
                .iter()
                .flat_map(|s| s.tasks.iter())
                .any(|t| t.id == *dep_id && t.status == TaskStatus::Done);
            if !dep_done {
                anyhow::bail!(
                    "Cannot start {}: dependency {} is not done",
                    task_id,
                    dep_id
                );
            }
        }
    }
    Ok(())
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn parse_status(s: &str) -> Result<TaskStatus> {
    match s {
        "pending" => Ok(TaskStatus::Pending),
        "in_progress" | "in-progress" => Ok(TaskStatus::InProgress),
        "done" => Ok(TaskStatus::Done),
        "blocked" => Ok(TaskStatus::Blocked),
        _ => anyhow::bail!(
            "Unknown task status: {}. Use: pending, in_progress, done, blocked",
            s
        ),
    }
}

// ── View models for JSON output ──────────────

#[derive(serde::Serialize)]
pub struct TaskView {
    pub stage_id: u32,
    pub id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_reason: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub meta: std::collections::HashMap<String, String>,
}

#[derive(serde::Serialize)]
struct TaskSummary {
    milestone: String,
    stages: Vec<StageSummary>,
}

#[derive(serde::Serialize)]
struct StageSummary {
    stage_id: u32,
    scope: String,
    total: usize,
    pending: usize,
    in_progress: usize,
    done: usize,
    blocked: usize,
}
