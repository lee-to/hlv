use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use colored::Colorize;
use serde_json;

use super::style;
use crate::model::milestone::{GateResult, GateRunStatus, MilestoneMap};
use crate::model::policy::{Gate, GatesPolicy};
use crate::model::project::ProjectMap;

pub fn run(project_root: &Path) -> Result<()> {
    run_show(project_root)
}

pub fn run_show(project_root: &Path) -> Result<()> {
    style::header("gates");

    let project = ProjectMap::load(&project_root.join("project.yaml"))?;
    let policy = GatesPolicy::load(&project_root.join(&project.paths.validation.gates_policy))?;

    // Show milestone context if available
    if let Ok(milestones) = MilestoneMap::load(&project_root.join("milestones.yaml")) {
        if let Some(ref current) = milestones.current {
            style::detail("Milestone", &current.id);
            if let Some(stage_num) = current.stage {
                if let Some(stage) = current.stages.iter().find(|s| s.id == stage_num) {
                    style::detail("Stage", &format!("{} ({})", stage_num, stage.status));
                }
            }
            println!();
        }
    }

    // Release policy
    if let Some(ref rp) = policy.release_policy {
        style::detail(
            "Release",
            &format!(
                "require_all_mandatory={}",
                if rp.require_all_mandatory {
                    "yes".green()
                } else {
                    "no".red()
                }
            ),
        );
        println!();
    }

    // Gate definitions
    println!(
        "  {:<25} {:<22} {:<10} {:<8} {:<10} {}",
        "Gate".bold(),
        "Type".bold(),
        "Mandatory".bold(),
        "Enabled".bold(),
        "Cwd".bold(),
        "Command".bold()
    );
    println!("  {}", "─".repeat(100));

    for gate in &policy.gates {
        let mandatory = if gate.mandatory {
            "yes".green()
        } else {
            "no".dimmed()
        };
        let enabled = if gate.enabled {
            "yes".green()
        } else {
            "off".dimmed()
        };
        let cwd = gate.cwd.as_deref().unwrap_or(".");
        let command = gate.command.as_deref().unwrap_or("—");

        println!(
            "  {:<25} {:<22} {:<10} {:<8} {:<10} {}",
            if gate.enabled {
                gate.id.normal()
            } else {
                gate.id.dimmed()
            },
            gate.gate_type.normal(),
            mandatory,
            enabled,
            cwd.dimmed(),
            if gate.command.is_some() {
                command.cyan()
            } else {
                command.dimmed()
            }
        );
    }

    let total = policy.gates.len();
    let enabled = policy.gates.iter().filter(|g| g.enabled).count();
    let with_cmd = policy.gates.iter().filter(|g| g.command.is_some()).count();

    println!(
        "\n  {} gates, {} enabled, {} with commands",
        total.to_string().bold(),
        enabled.to_string().green(),
        with_cmd.to_string().cyan()
    );

    Ok(())
}

pub fn run_enable(project_root: &Path, gate_id: &str) -> Result<()> {
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;
    let policy_path = project_root.join(&project.paths.validation.gates_policy);
    let mut policy = GatesPolicy::load(&policy_path)?;

    let gate = policy
        .find_gate_mut(gate_id)
        .context(format!("Gate '{}' not found", gate_id))?;
    gate.enabled = true;
    policy.save(&policy_path)?;

    style::ok(&format!("{} enabled", gate_id));
    Ok(())
}

pub fn run_disable(project_root: &Path, gate_id: &str) -> Result<()> {
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;
    let policy_path = project_root.join(&project.paths.validation.gates_policy);
    let mut policy = GatesPolicy::load(&policy_path)?;

    let gate = policy
        .find_gate_mut(gate_id)
        .context(format!("Gate '{}' not found", gate_id))?;
    gate.enabled = false;
    policy.save(&policy_path)?;

    style::ok(&format!("{} disabled", gate_id));
    Ok(())
}

pub fn run_set_command(project_root: &Path, gate_id: &str, command: &str) -> Result<()> {
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;
    let policy_path = project_root.join(&project.paths.validation.gates_policy);
    let mut policy = GatesPolicy::load(&policy_path)?;

    let gate = policy
        .find_gate_mut(gate_id)
        .context(format!("Gate '{}' not found", gate_id))?;
    gate.command = Some(command.to_string());
    policy.save(&policy_path)?;

    style::ok(&format!("{} command set: {}", gate_id, command.cyan()));
    Ok(())
}

pub fn run_clear_command(project_root: &Path, gate_id: &str) -> Result<()> {
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;
    let policy_path = project_root.join(&project.paths.validation.gates_policy);
    let mut policy = GatesPolicy::load(&policy_path)?;

    let gate = policy
        .find_gate_mut(gate_id)
        .context(format!("Gate '{}' not found", gate_id))?;
    gate.command = None;
    policy.save(&policy_path)?;

    style::ok(&format!("{} command cleared", gate_id));
    Ok(())
}

pub fn run_set_cwd(project_root: &Path, gate_id: &str, cwd: &str) -> Result<()> {
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;
    let policy_path = project_root.join(&project.paths.validation.gates_policy);
    let mut policy = GatesPolicy::load(&policy_path)?;

    let gate = policy
        .find_gate_mut(gate_id)
        .context(format!("Gate '{}' not found", gate_id))?;
    gate.cwd = Some(cwd.to_string());
    policy.save(&policy_path)?;

    style::ok(&format!("{} cwd set: {}", gate_id, cwd.cyan()));
    Ok(())
}

pub fn run_clear_cwd(project_root: &Path, gate_id: &str) -> Result<()> {
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;
    let policy_path = project_root.join(&project.paths.validation.gates_policy);
    let mut policy = GatesPolicy::load(&policy_path)?;

    let gate = policy
        .find_gate_mut(gate_id)
        .context(format!("Gate '{}' not found", gate_id))?;
    gate.cwd = None;
    policy.save(&policy_path)?;

    style::ok(&format!("{} cwd cleared (will use project root)", gate_id));
    Ok(())
}

pub fn run_show_json(project_root: &Path) -> Result<()> {
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;
    let policy = GatesPolicy::load(&project_root.join(&project.paths.validation.gates_policy))?;

    let gates_json: Vec<serde_json::Value> = policy
        .gates
        .iter()
        .map(|g| {
            serde_json::json!({
                "id": g.id,
                "type": g.gate_type,
                "mandatory": g.mandatory,
                "enabled": g.enabled,
                "command": g.command,
                "cwd": g.cwd,
            })
        })
        .collect();

    println!("{}", serde_json::to_string_pretty(&gates_json)?);
    Ok(())
}

pub fn run_add(
    project_root: &Path,
    id: &str,
    gate_type: &str,
    mandatory: bool,
    command: Option<&str>,
    cwd: Option<&str>,
    enabled: bool,
) -> Result<()> {
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;
    let policy_path = project_root.join(&project.paths.validation.gates_policy);
    let mut policy = GatesPolicy::load(&policy_path)?;

    let gate = Gate {
        id: id.to_string(),
        gate_type: gate_type.to_string(),
        mandatory,
        enabled,
        pass_criteria: None,
        command: command.map(|s| s.to_string()),
        cwd: cwd.map(|s| s.to_string()),
        tools: None,
    };

    policy.add_gate(gate)?;
    policy.save(&policy_path)?;

    style::ok(&format!(
        "{} added (type={}, mandatory={})",
        id, gate_type, mandatory
    ));
    Ok(())
}

pub fn run_remove(project_root: &Path, id: &str, force: bool) -> Result<()> {
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;
    let policy_path = project_root.join(&project.paths.validation.gates_policy);
    let mut policy = GatesPolicy::load(&policy_path)?;

    // Check if gate exists and is mandatory
    let gate = policy
        .gates
        .iter()
        .find(|g| g.id == id)
        .ok_or_else(|| anyhow::anyhow!("Gate '{}' not found", id))?;

    if gate.mandatory && !force {
        print!(
            "  {} This gate is mandatory. Remove anyway? [y/N] ",
            "!".yellow().bold()
        );
        io::stdout().flush()?;
        let stdin = io::stdin();
        let line = stdin.lock().lines().next().unwrap_or(Ok(String::new()))?;
        if !line.trim().eq_ignore_ascii_case("y") {
            style::hint("Cancelled");
            return Ok(());
        }
    }

    policy.remove_gate(id)?;
    policy.save(&policy_path)?;

    style::ok(&format!("{} removed", id));
    Ok(())
}

pub fn run_edit(
    project_root: &Path,
    id: &str,
    gate_type: Option<&str>,
    mandatory: bool,
    no_mandatory: bool,
) -> Result<()> {
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;
    let policy_path = project_root.join(&project.paths.validation.gates_policy);
    let mut policy = GatesPolicy::load(&policy_path)?;

    let gate = policy
        .find_gate_mut(id)
        .context(format!("Gate '{}' not found", id))?;

    if let Some(t) = gate_type {
        gate.gate_type = t.to_string();
    }
    if mandatory {
        gate.mandatory = true;
    } else if no_mandatory {
        gate.mandatory = false;
    }

    policy.save(&policy_path)?;

    style::ok(&format!("{} updated", id));
    Ok(())
}

/// Run all enabled gates that have commands. Returns (passed, failed, skipped).
/// If `filter_id` is Some, only run the specified gate.
pub fn run_gate_commands(project_root: &Path, filter_id: Option<&str>) -> Result<(u32, u32, u32)> {
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;
    let policy = GatesPolicy::load(&project_root.join(&project.paths.validation.gates_policy))?;

    let runnable: Vec<_> = policy
        .gates
        .iter()
        .filter(|g| g.enabled && g.command.is_some())
        .filter(|g| match filter_id {
            Some(fid) => g.id == fid,
            None => true,
        })
        .collect();

    if runnable.is_empty() {
        return Ok((0, 0, 0));
    }

    style::section("Gates");

    let quiet = style::is_quiet();
    let mut passed = 0u32;
    let mut failed = 0u32;
    let skipped = 0u32;
    let mut gate_results: Vec<GateResult> = Vec::new();
    let now = chrono::Local::now().to_rfc3339();

    for gate in &runnable {
        let cmd = gate.command.as_deref().unwrap();
        let work_dir = match &gate.cwd {
            Some(rel) => project_root.join(rel),
            None => project_root.to_path_buf(),
        };
        if !quiet {
            let cwd_label = gate.cwd.as_deref().unwrap_or(".");
            print!(
                "  {} {} {}",
                gate.id.bold(),
                cmd.dimmed(),
                format!("({})", cwd_label).dimmed()
            );
            std::io::Write::flush(&mut std::io::stdout())?;
        }

        let result = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(&work_dir)
            .output();

        match result {
            Ok(output) if output.status.success() => {
                if !quiet {
                    println!("{}", "PASSED".green());
                }
                passed += 1;
                gate_results.push(GateResult {
                    id: gate.id.clone(),
                    status: GateRunStatus::Passed,
                    run_at: Some(now.clone()),
                });
            }
            Ok(output) => {
                if !quiet {
                    println!("{}", "FAILED".red());
                }
                failed += 1;
                gate_results.push(GateResult {
                    id: gate.id.clone(),
                    status: GateRunStatus::Failed,
                    run_at: Some(now.clone()),
                });
                if !quiet {
                    // Show last lines of stderr/stdout for diagnostics
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout_str = String::from_utf8_lossy(&output.stdout);
                    let output_text = if !stderr.is_empty() {
                        stderr.to_string()
                    } else {
                        stdout_str.to_string()
                    };
                    for line in output_text
                        .lines()
                        .rev()
                        .take(5)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                    {
                        println!("    {}", line.dimmed());
                    }
                }
            }
            Err(e) => {
                if !quiet {
                    println!("{} ({})", "FAILED".red(), e);
                }
                failed += 1;
                gate_results.push(GateResult {
                    id: gate.id.clone(),
                    status: GateRunStatus::Failed,
                    run_at: Some(now.clone()),
                });
            }
        }
    }

    if !quiet {
        println!(
            "\n  Gates: {} passed, {} failed, {} skipped",
            passed.to_string().green(),
            failed.to_string().red(),
            skipped.to_string().dimmed()
        );
    }

    // Save gate results to milestones.yaml
    save_gate_results(project_root, &gate_results);

    Ok((passed, failed, skipped))
}

/// Persist gate run results into milestones.yaml (current milestone).
fn save_gate_results(project_root: &Path, results: &[GateResult]) {
    if results.is_empty() {
        return;
    }
    let ms_path = project_root.join("milestones.yaml");
    let Ok(mut milestones) = MilestoneMap::load(&ms_path) else {
        return;
    };
    let Some(ref mut current) = milestones.current else {
        return;
    };
    current.gate_results = results.to_vec();
    let _ = milestones.save(&ms_path);
}
