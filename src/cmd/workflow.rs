use std::path::Path;

use anyhow::Result;
use colored::Colorize;

use super::style;
use crate::model::milestone::{MilestoneMap, StageStatus};
use crate::model::task::TaskStatus;

struct PhaseInfo {
    name: &'static str,
    who: &'static str,
}

const PHASES: &[PhaseInfo] = &[
    PhaseInfo {
        name: "Bootstrap",
        who: "you",
    },
    PhaseInfo {
        name: "Artifacts",
        who: "you",
    },
    PhaseInfo {
        name: "Generate",
        who: "LLM + you",
    },
    PhaseInfo {
        name: "Verify",
        who: "machine",
    },
    PhaseInfo {
        name: "Implement",
        who: "LLM",
    },
    PhaseInfo {
        name: "Validate",
        who: "machine",
    },
];

#[derive(serde::Serialize)]
pub struct WorkflowData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestone_id: Option<String>,
    pub phase: u8,
    pub phase_name: String,
    pub stages: Vec<WorkflowStageData>,
    pub next_actions: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct WorkflowStageData {
    pub id: u32,
    pub scope: String,
    pub status: String,
    pub active: bool,
    pub task_count: usize,
    pub tasks_done: usize,
}

pub fn get_workflow(project_root: &Path) -> Result<WorkflowData> {
    let milestones = MilestoneMap::load(&project_root.join("milestones.yaml"))?;
    let current = match &milestones.current {
        None => {
            return Ok(WorkflowData {
                milestone_id: None,
                phase: 0,
                phase_name: "No milestone".to_string(),
                stages: Vec::new(),
                next_actions: vec!["hlv milestone new <name>".to_string()],
            })
        }
        Some(c) => c,
    };

    if current.stages.is_empty() {
        return Ok(WorkflowData {
            milestone_id: Some(current.id.clone()),
            phase: 1,
            phase_name: "Artifacts".to_string(),
            stages: Vec::new(),
            next_actions: vec![
                "Add artifacts or run /artifacts".to_string(),
                "Then run /generate".to_string(),
            ],
        });
    }

    let active_stage = current
        .stage
        .and_then(|id| current.stages.iter().find(|s| s.id == id))
        .or_else(|| {
            current
                .stages
                .iter()
                .find(|s| s.status != StageStatus::Validated)
        });

    let phase = match active_stage {
        Some(s) => match s.status {
            StageStatus::Pending => 2,
            StageStatus::Verified => 3,
            StageStatus::Implementing => 4,
            StageStatus::Implemented => 4,
            StageStatus::Validating => 5,
            StageStatus::Validated => 5,
        },
        None => 5,
    };

    let phase_name = match phase {
        1 => "Artifacts",
        2 => "Generate",
        3 => "Verify",
        4 => "Implement",
        5 => "Validate",
        _ => "Unknown",
    }
    .to_string();

    let stages = current
        .stages
        .iter()
        .map(|s| {
            let task_count = s.tasks.len();
            let tasks_done = s
                .tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Done)
                .count();
            WorkflowStageData {
                id: s.id,
                scope: s.scope.clone(),
                status: s.status.to_string(),
                active: current.stage == Some(s.id),
                task_count,
                tasks_done,
            }
        })
        .collect();

    let mut next_actions = Vec::new();
    if let Some(s) = active_stage {
        match s.status {
            StageStatus::Pending | StageStatus::Verified => {
                next_actions.push("Run /implement to start this stage".to_string());
            }
            StageStatus::Implementing => {
                next_actions.push("Implementation in progress".to_string());
                if !s.tasks.is_empty() && s.tasks.iter().all(|t| t.status == TaskStatus::Done) {
                    next_actions.push("All tasks done — consider advancing the stage".to_string());
                }
            }
            StageStatus::Implemented => {
                next_actions.push("Run /validate".to_string());
                next_actions.push("Found issues? → hlv stage reopen or hlv task add".to_string());
            }
            StageStatus::Validating => {
                next_actions.push("Run /implement for remediation".to_string());
                if !s.tasks.is_empty() && s.tasks.iter().all(|t| t.status == TaskStatus::Done) {
                    next_actions.push("All tasks done — consider advancing the stage".to_string());
                }
            }
            StageStatus::Validated => {
                if current
                    .stages
                    .iter()
                    .all(|st| st.status == StageStatus::Validated)
                {
                    next_actions.push("All stages validated — run hlv milestone done".to_string());
                }
            }
        }
    } else {
        next_actions.push("All stages validated — run hlv milestone done".to_string());
    }

    Ok(WorkflowData {
        milestone_id: Some(current.id.clone()),
        phase,
        phase_name,
        stages,
        next_actions,
    })
}

pub fn run(project_root: &Path, json: bool) -> Result<()> {
    if json {
        let data = get_workflow(project_root)?;
        println!("{}", serde_json::to_string_pretty(&data)?);
        return Ok(());
    }
    run_milestone(project_root)
}

fn run_milestone(project_root: &Path) -> Result<()> {
    let milestones = MilestoneMap::load(&project_root.join("milestones.yaml"))?;

    style::header("workflow");

    let current = match &milestones.current {
        None => {
            println!("    No active milestone.");
            println!();
            style::section("What to do next");
            action("hlv milestone new <name> — start a new milestone");
            println!();
            return Ok(());
        }
        Some(c) => c,
    };

    style::detail("Milestone", &current.id);

    if current.stages.is_empty() {
        // No stages yet — need artifacts + generate
        print_milestone_diagram(0);
        println!();
        style::section("You are here");
        println!("    Phase 1: {} (you)", "Artifacts".bold());
        println!();
        style::section("What to do next");
        println!(
            "    {} Add artifacts to human/milestones/{}/artifacts/",
            "→".cyan(),
            current.id
        );
        println!(
            "    {} Or run /artifacts for interactive interview",
            "→".cyan()
        );
        println!(
            "    {} Then run /generate to create contracts + stages",
            "→".cyan()
        );
        println!();
        return Ok(());
    }

    // Find current stage — use explicit pointer or infer from first non-validated
    let active_stage = current
        .stage
        .and_then(|id| current.stages.iter().find(|s| s.id == id))
        .or_else(|| {
            current
                .stages
                .iter()
                .find(|s| s.status != StageStatus::Validated)
        });

    // Determine phase from stage status
    let phase = match active_stage {
        Some(s) => match s.status {
            StageStatus::Pending => 2,
            StageStatus::Verified => 3,
            StageStatus::Implementing => 4,
            StageStatus::Implemented => 4,
            StageStatus::Validating => 5,
            StageStatus::Validated => 5,
        },
        None => 5, // all validated or no stages
    };

    print_milestone_diagram(phase);
    println!();

    style::section("You are here");
    if (phase as usize) < PHASES.len() {
        let p = &PHASES[phase as usize];
        println!(
            "    Phase {}: {} ({})",
            phase,
            p.name.bold(),
            p.who.dimmed()
        );
    }

    // Stage overview
    style::section("Stages");
    for s in &current.stages {
        let icon = match s.status {
            StageStatus::Validated => "✓".green(),
            StageStatus::Verified => "✓".green(),
            StageStatus::Implementing | StageStatus::Validating => "▸".yellow(),
            StageStatus::Implemented => "●".cyan(),
            StageStatus::Pending => "○".dimmed(),
        };
        let active = if current.stage == Some(s.id) {
            " ◀"
        } else {
            ""
        };
        let task_info = if !s.tasks.is_empty() {
            let done = s
                .tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Done)
                .count();
            format!(" ({}/{} tasks done)", done, s.tasks.len())
        } else {
            String::new()
        };
        println!(
            "    {} Stage {}: {} [{}]{}{}",
            icon,
            s.id,
            s.scope,
            super::status::colorize_status(&s.status.to_string()),
            task_info.dimmed(),
            active.cyan()
        );
    }

    println!();
    style::section("What to do next");
    print_milestone_next_actions(current);

    println!();
    Ok(())
}

fn print_milestone_diagram(current_phase: u8) {
    println!();
    let milestone_phases = [
        ("1", "Artifacts", "/artifacts"),
        ("2", "Generate", "/generate"),
        ("3", "Verify", "/verify"),
        ("4", "Implement", "/implement"),
        ("5", "Validate", "/validate"),
    ];

    let mut names = String::new();
    for (i, (num, name, _cmd)) in milestone_phases.iter().enumerate() {
        let phase_num = (i + 1) as u8;
        let formatted = if phase_num == current_phase {
            format!("[{}:{}]", num, name).bold().cyan().to_string()
        } else if phase_num < current_phase {
            format!(" {}:{} ", num, name).green().to_string()
        } else {
            format!(" {}:{} ", num, name).dimmed().to_string()
        };
        names.push_str(&formatted);
        if i < milestone_phases.len() - 1 {
            let arrow = if phase_num < current_phase {
                " --> ".green().to_string()
            } else if phase_num == current_phase {
                " --> ".cyan().to_string()
            } else {
                " --> ".dimmed().to_string()
            };
            names.push_str(&arrow);
        }
    }
    println!("    {}", names);
}

fn print_milestone_next_actions(current: &crate::model::milestone::MilestoneCurrent) {
    let active_stage = current
        .stage
        .and_then(|id| current.stages.iter().find(|s| s.id == id))
        .or_else(|| {
            // No stage pointer — infer from first non-validated stage
            current
                .stages
                .iter()
                .find(|s| s.status != StageStatus::Validated)
        });

    match active_stage {
        Some(s) => {
            match s.status {
                StageStatus::Pending => {
                    action("Run /implement to start this stage");
                }
                StageStatus::Verified => {
                    action("Run /implement to start this stage");
                }
                StageStatus::Implementing => {
                    action("Implementation in progress — wait for /implement to finish");
                    hint("Or run /implement to continue if interrupted");
                }
                StageStatus::Implemented => {
                    action("Run /validate to prove correctness of this stage");
                    hint("Found issues? → hlv stage reopen <N> or hlv task add --stage <N> <ID> <name>");
                }
                StageStatus::Validating => {
                    action("Validation found issues — run /implement for remediation tasks");
                    hint("Then /validate again to re-check");
                }
                StageStatus::Validated => {
                    // Current stage validated — check if more stages
                    let next = current
                        .stages
                        .iter()
                        .find(|ns| ns.status != StageStatus::Validated);
                    match next {
                        Some(ns) => {
                            action(&format!(
                                "Stage {} validated. Advance to stage {}: {}",
                                s.id, ns.id, ns.scope
                            ));
                            hint("Run /implement (new context window) for next stage");
                        }
                        None => {
                            let all_validated = current
                                .stages
                                .iter()
                                .all(|st| st.status == StageStatus::Validated);
                            if all_validated {
                                println!(
                                    "    {} {}",
                                    "✓".green().bold(),
                                    "All stages validated — run `hlv milestone done`"
                                        .green()
                                        .bold()
                                );
                            } else {
                                action("Some stages still need work");
                            }
                        }
                    }
                }
            }
        }
        None => {
            // All stages validated (or no stages)
            println!(
                "    {} {}",
                "✓".green().bold(),
                "All stages validated — run `hlv milestone done`"
                    .green()
                    .bold()
            );
        }
    }
}

fn action(msg: &str) {
    println!("    {} {}", "→".cyan(), msg);
}

fn hint(msg: &str) {
    println!("      {}", msg.dimmed());
}
