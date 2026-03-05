use std::path::Path;

use crate::check::Diagnostic;
use crate::model::milestone::MilestoneMap;
use crate::model::stage::StagePlan;
use crate::model::task::TaskStatus;

/// Check task-related diagnostics (TSK-010..050).
pub fn check_tasks(root: &Path) -> Vec<Diagnostic> {
    let milestones_path = root.join("milestones.yaml");
    if !milestones_path.exists() {
        return Vec::new();
    }
    let map = match MilestoneMap::load(&milestones_path) {
        Ok(m) => m,
        Err(_) => return Vec::new(), // MST-001 already covers parse errors
    };
    let current = match map.current.as_ref() {
        Some(c) => c,
        None => return Vec::new(),
    };

    let milestone_dir = root.join("human/milestones").join(&current.id);
    let mut diags = Vec::new();

    for stage in &current.stages {
        // Load stage plan if exists
        let stage_path = milestone_dir.join(format!("stage_{}.md", stage.id));
        let plan = if stage_path.exists() {
            StagePlan::load(&stage_path).ok()
        } else {
            None
        };

        let plan_tasks: Vec<_> = plan
            .as_ref()
            .map(|p| p.tasks.iter().chain(p.remediation.iter()).collect())
            .unwrap_or_default();

        for task in &stage.tasks {
            // TSK-010: task InProgress longer than 7 days
            if task.status == TaskStatus::InProgress {
                if let Some(ref started) = task.started_at {
                    if let Ok(started_dt) = chrono::DateTime::parse_from_rfc3339(started) {
                        let days = (chrono::Utc::now() - started_dt.with_timezone(&chrono::Utc))
                            .num_days();
                        if days > 7 {
                            diags.push(Diagnostic::warning(
                                "TSK-010",
                                format!(
                                    "Task {} in stage {} has been in_progress for {} days",
                                    task.id, stage.id, days
                                ),
                            ));
                        }
                    }
                }
            }

            // TSK-020: task Done but output files don't exist
            if task.status == TaskStatus::Done {
                if let Some(plan_task) = plan_tasks.iter().find(|pt| pt.id == task.id) {
                    for output in &plan_task.output {
                        let output_path = root.join(output);
                        if !output_path.exists() {
                            diags.push(
                                Diagnostic::error(
                                    "TSK-020",
                                    format!(
                                        "Task {} is done but output path does not exist: {}",
                                        task.id, output
                                    ),
                                )
                                .with_file(output),
                            );
                        }
                    }
                }
            }

            // TSK-040: task InProgress but dependency not Done (cross-stage)
            if task.status == TaskStatus::InProgress {
                if let Some(plan_task) = plan_tasks.iter().find(|pt| pt.id == task.id) {
                    for dep_id in &plan_task.depends_on {
                        // Search across ALL stages for the dependency
                        let dep_done = current
                            .stages
                            .iter()
                            .flat_map(|s| s.tasks.iter())
                            .any(|t| t.id == *dep_id && t.status == TaskStatus::Done);
                        if !dep_done {
                            diags.push(Diagnostic::error(
                                "TSK-040",
                                format!(
                                    "Task {} is in_progress but dependency {} is not done (stage {})",
                                    task.id, dep_id, stage.id
                                ),
                            ));
                        }
                    }
                }
            }

            // TSK-050: tracker in yaml but task not in stage_N.md
            if let Some(ref plan) = plan {
                let all_plan_ids: Vec<&str> = plan
                    .tasks
                    .iter()
                    .chain(plan.remediation.iter())
                    .map(|t| t.id.as_str())
                    .collect();
                if !all_plan_ids.contains(&task.id.as_str()) {
                    diags.push(Diagnostic::warning(
                        "TSK-050",
                        format!(
                            "Task {} in stage {} tracker but not in stage_{}.md — run `hlv task sync`",
                            task.id, stage.id, stage.id
                        ),
                    ));
                }
            }
        }

        // TSK-030: all tasks Done but stage not Implemented/Validated
        if !stage.tasks.is_empty() {
            let all_done = stage.tasks.iter().all(|t| t.status == TaskStatus::Done);
            if all_done {
                use crate::model::milestone::StageStatus;
                let stage_advanced = matches!(
                    stage.status,
                    StageStatus::Implemented | StageStatus::Validating | StageStatus::Validated
                );
                if !stage_advanced {
                    diags.push(Diagnostic::warning(
                        "TSK-030",
                        format!(
                            "All tasks in stage {} are done but stage status is '{}' — consider advancing",
                            stage.id, stage.status
                        ),
                    ));
                }
            }
        }
    }

    diags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tsk030_all_done_stage_not_advanced() {
        use crate::model::milestone::{MilestoneCurrent, MilestoneMap, StageEntry, StageStatus};
        use crate::model::task::TaskTracker;
        use std::collections::HashMap;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("human/milestones/001")).unwrap();

        let mut t1 = TaskTracker::new("TASK-001".to_string());
        t1.status = TaskStatus::Done;
        t1.completed_at = Some("2026-03-08T10:00:00Z".to_string());

        let map = MilestoneMap {
            project: "test".to_string(),
            current: Some(MilestoneCurrent {
                id: "001".to_string(),
                number: 1,
                branch: None,
                stage: None,
                stages: vec![StageEntry {
                    id: 1,
                    scope: "Test".to_string(),
                    status: StageStatus::Implementing,
                    commit: None,
                    tasks: vec![t1],
                    labels: Vec::new(),
                    meta: HashMap::new(),
                }],
                gate_results: Default::default(),
                git: None,
                labels: Vec::new(),
                meta: HashMap::new(),
            }),
            history: Vec::new(),
        };
        map.save(&root.join("milestones.yaml")).unwrap();

        let diags = check_tasks(root);
        assert!(
            diags.iter().any(|d| d.code == "TSK-030"),
            "Expected TSK-030, got: {:?}",
            diags
        );
    }
}
