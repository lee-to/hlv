use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use colored::Colorize;
use serde_json;

use super::style;
use crate::check::Diagnostic;
use crate::model::milestone::{GateResult, GateRunStatus, MilestoneMap};
use crate::model::policy::{Gate, GatesPolicy};
use crate::model::project::ProjectMap;
use crate::util::command_parser::{gate_command_failure_reason, parse_portable_command};
use crate::util::cwd::ensure_existing_cwd;
use crate::util::display_width::{pad_display_width, truncate_display_width};

#[derive(Debug, Clone, serde::Serialize)]
pub struct GateCommandRunSummary {
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub results: Vec<GateCommandRunResult>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GateCommandRunResult {
    pub id: String,
    pub status: GateRunStatus,
    pub reason: String,
    pub cwd: String,
    pub command: Option<String>,
}

pub fn run(project_root: &Path) -> Result<()> {
    let project_root = &crate::config_root(project_root);
    run_show(project_root)
}

pub fn run_show(project_root: &Path) -> Result<()> {
    let project_root = &crate::config_root(project_root);
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
        "  {} {} {} {} {} {}",
        table_cell("Gate", 25).bold(),
        table_cell("Type", 22).bold(),
        table_cell("Mandatory", 10).bold(),
        table_cell("Enabled", 8).bold(),
        table_cell("Cwd", 10).bold(),
        "Command".bold()
    );
    println!("  {}", "─".repeat(100));

    for gate in &policy.gates {
        let cwd = gate.cwd.as_deref().unwrap_or(".");
        let command = gate.command.as_deref().unwrap_or("—");

        let gate_id = table_cell(&gate.id, 25);
        let gate_type = table_cell(&gate.gate_type, 22);
        let cwd = table_cell(cwd, 10);
        let mandatory = table_cell(if gate.mandatory { "yes" } else { "no" }, 10);
        let enabled = table_cell(if gate.enabled { "yes" } else { "off" }, 8);

        println!(
            "  {} {} {} {} {} {}",
            if gate.enabled {
                gate_id.normal()
            } else {
                gate_id.dimmed()
            },
            gate_type.normal(),
            if gate.mandatory {
                mandatory.green()
            } else {
                mandatory.dimmed()
            },
            if gate.enabled {
                enabled.green()
            } else {
                enabled.dimmed()
            },
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

fn table_cell(s: &str, width: usize) -> String {
    pad_display_width(&truncate_display_width(s, width), width)
}

pub fn run_enable(project_root: &Path, gate_id: &str) -> Result<()> {
    let project_root = &crate::config_root(project_root);
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
    let project_root = &crate::config_root(project_root);
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
    let project_root = &crate::config_root(project_root);
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;
    let policy_path = project_root.join(&project.paths.validation.gates_policy);
    let mut policy = GatesPolicy::load(&policy_path)?;

    let gate = policy
        .find_gate_mut(gate_id)
        .context(format!("Gate '{}' not found", gate_id))?;
    validate_gate_command(command)?;
    gate.command = Some(command.to_string());
    policy.save(&policy_path)?;

    style::ok(&format!("{} command set: {}", gate_id, command.cyan()));
    Ok(())
}

pub fn run_clear_command(project_root: &Path, gate_id: &str) -> Result<()> {
    let project_root = &crate::config_root(project_root);
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
    let project_root = &crate::config_root(project_root);
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;
    let policy_path = project_root.join(&project.paths.validation.gates_policy);
    let mut policy = GatesPolicy::load(&policy_path)?;

    let gate = policy
        .find_gate_mut(gate_id)
        .context(format!("Gate '{}' not found", gate_id))?;
    ensure_existing_cwd(project_root, Some(cwd), &format!("Gate '{}' cwd", gate_id))?;
    gate.cwd = Some(cwd.to_string());
    policy.save(&policy_path)?;

    style::ok(&format!("{} cwd set: {}", gate_id, cwd.cyan()));
    Ok(())
}

pub fn run_clear_cwd(project_root: &Path, gate_id: &str) -> Result<()> {
    let project_root = &crate::config_root(project_root);
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
    let project_root = &crate::config_root(project_root);
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

#[derive(Debug, Clone)]
pub struct GateCommandReport {
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub diagnostics: Vec<Diagnostic>,
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
    let project_root = &crate::config_root(project_root);
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;
    let policy_path = project_root.join(&project.paths.validation.gates_policy);
    let mut policy = GatesPolicy::load(&policy_path)?;

    if let Some(cmd) = command {
        validate_gate_command(cmd)?;
    }
    if let Some(rel) = cwd {
        ensure_existing_cwd(project_root, Some(rel), &format!("Gate '{}' cwd", id))?;
    }

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
    let project_root = &crate::config_root(project_root);
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
    let project_root = &crate::config_root(project_root);
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

/// Run gates and return only aggregate counters.
/// If `filter_id` is Some, only process the specified gate.
pub fn run_gate_commands(project_root: &Path, filter_id: Option<&str>) -> Result<(u32, u32, u32)> {
    let summary = run_gate_commands_with_results(project_root, filter_id, !style::is_quiet())?;
    Ok((summary.passed, summary.failed, summary.skipped))
}

/// Run enabled gate commands and return machine-readable diagnostics for failures.
/// If `emit_output` is false, no progress output is printed.
pub fn run_gate_command_report(
    project_root: &Path,
    filter_id: Option<&str>,
    emit_output: bool,
) -> Result<GateCommandReport> {
    let project = ProjectMap::load(&crate::config_root(project_root).join("project.yaml"))?;
    let gates_policy_path = project.paths.validation.gates_policy.clone();
    let summary = run_gate_commands_with_results(project_root, filter_id, emit_output)?;
    let diagnostics = summary
        .results
        .iter()
        .filter(|result| matches!(result.status, GateRunStatus::Failed))
        .map(|result| {
            Diagnostic::error(
                "GAT-050",
                format!("Gate '{}' command failed: {}", result.id, result.reason),
            )
            .with_file(&gates_policy_path)
        })
        .collect();

    Ok(GateCommandReport {
        passed: summary.passed,
        failed: summary.failed,
        skipped: summary.skipped,
        diagnostics,
    })
}

/// Run gates and return structured per-gate results.
/// If `filter_id` is Some, only process the specified gate.
pub fn run_gate_commands_with_results(
    project_root: &Path,
    filter_id: Option<&str>,
    emit_human: bool,
) -> Result<GateCommandRunSummary> {
    // Policy is a config artifact (config root); gate commands execute
    // relative to the repository root (`project_root`).
    let config_root = crate::config_root(project_root);
    let project = ProjectMap::load(&config_root.join("project.yaml"))?;
    let policy = GatesPolicy::load(&config_root.join(&project.paths.validation.gates_policy))?;

    let selected: Vec<_> = policy
        .gates
        .iter()
        .filter(|g| match filter_id {
            Some(fid) => g.id == fid,
            None => true,
        })
        .collect();

    if selected.is_empty() {
        return Ok(GateCommandRunSummary {
            passed: 0,
            failed: 0,
            skipped: 0,
            results: vec![],
        });
    }

    if emit_human {
        style::section("Gates");
    }

    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;
    let mut results: Vec<GateCommandRunResult> = Vec::new();
    let now = chrono::Local::now().to_rfc3339();

    for gate in &selected {
        let command = gate.command.clone();
        let cwd_label = gate.cwd.as_deref().unwrap_or(".").to_string();

        if !gate.enabled {
            if emit_human {
                print_gate_start(gate, command.as_deref(), &cwd_label)?;
                println!("{}", "SKIPPED".yellow());
                println!("    {}", "reason: disabled".dimmed());
            }
            skipped += 1;
            results.push(GateCommandRunResult {
                id: gate.id.clone(),
                status: GateRunStatus::Skipped,
                reason: "disabled".to_string(),
                cwd: cwd_label,
                command,
            });
            continue;
        }

        let Some(cmd) = command.as_deref() else {
            if emit_human {
                print_gate_start(gate, None, &cwd_label)?;
                println!("{}", "SKIPPED".yellow());
                println!("    {}", "reason: no command".dimmed());
            }
            skipped += 1;
            results.push(GateCommandRunResult {
                id: gate.id.clone(),
                status: GateRunStatus::Skipped,
                reason: "no command".to_string(),
                cwd: cwd_label,
                command: None,
            });
            continue;
        };

        if emit_human {
            print_gate_start(gate, Some(cmd), &cwd_label)?;
        }

        let parsed = parse_portable_command(cmd);

        match parsed {
            Ok(parsed_cmd) => {
                let work_dir = match ensure_existing_cwd(
                    project_root,
                    gate.cwd.as_deref(),
                    &format!("Gate '{}' cwd", gate.id),
                ) {
                    Ok((work_dir, _)) => work_dir,
                    Err(e) => {
                        let reason = e.to_string();
                        if emit_human {
                            println!("{}", "FAILED".red());
                            println!("    {}", format!("reason: {}", reason).dimmed());
                        }
                        failed += 1;
                        results.push(GateCommandRunResult {
                            id: gate.id.clone(),
                            status: GateRunStatus::Failed,
                            reason,
                            cwd: cwd_label,
                            command,
                        });
                        continue;
                    }
                };

                let result = Command::new(&parsed_cmd.program)
                    .args(&parsed_cmd.args)
                    .current_dir(&work_dir)
                    .output();

                match result {
                    Ok(output) if output.status.success() => {
                        if emit_human {
                            println!("{}", "PASSED".green());
                        }
                        passed += 1;
                        results.push(GateCommandRunResult {
                            id: gate.id.clone(),
                            status: GateRunStatus::Passed,
                            reason: "ok".to_string(),
                            cwd: cwd_label,
                            command,
                        });
                    }
                    Ok(output) => {
                        let status_text = output
                            .status
                            .code()
                            .map(|code| format!("exit code {}", code))
                            .unwrap_or_else(|| "terminated by signal".to_string());
                        if emit_human {
                            println!("{}", "FAILED".red());
                            println!("    {}", format!("reason: {}", status_text).dimmed());
                        }
                        failed += 1;
                        results.push(GateCommandRunResult {
                            id: gate.id.clone(),
                            status: GateRunStatus::Failed,
                            reason: status_text,
                            cwd: cwd_label,
                            command,
                        });
                        if emit_human {
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
                        let reason = format!("failed to start executable: {}", e);
                        if emit_human {
                            println!("{}", "FAILED".red());
                            println!("    {}", format!("reason: {}", reason).dimmed());
                        }
                        failed += 1;
                        results.push(GateCommandRunResult {
                            id: gate.id.clone(),
                            status: GateRunStatus::Failed,
                            reason,
                            cwd: cwd_label,
                            command,
                        });
                    }
                }
            }
            Err(e) => {
                let reason = gate_command_failure_reason(&e);
                if emit_human {
                    println!("{}", "FAILED".red());
                    println!("    {}", format!("reason: {}", reason).dimmed());
                }
                failed += 1;
                results.push(GateCommandRunResult {
                    id: gate.id.clone(),
                    status: GateRunStatus::Failed,
                    reason,
                    cwd: cwd_label,
                    command,
                });
            }
        }
    }

    if emit_human {
        println!(
            "\n  Gates: {} passed, {} failed, {} skipped",
            passed.to_string().green(),
            failed.to_string().red(),
            skipped.to_string().dimmed()
        );
    }

    let gate_results: Vec<GateResult> = results
        .iter()
        .map(|result| GateResult {
            id: result.id.clone(),
            status: result.status.clone(),
            run_at: Some(now.clone()),
        })
        .collect();
    save_gate_results(project_root, &gate_results);

    Ok(GateCommandRunSummary {
        passed,
        failed,
        skipped,
        results,
    })
}

fn validate_gate_command(command: &str) -> Result<()> {
    parse_portable_command(command)
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!(gate_command_failure_reason(&e)))
}

fn print_gate_start(gate: &Gate, command: Option<&str>, cwd_label: &str) -> Result<()> {
    let command_label = command.unwrap_or("-");
    print!(
        "  {} {} {}",
        gate.id.bold(),
        command_label.dimmed(),
        format!("({})", cwd_label).dimmed()
    );
    std::io::Write::flush(&mut std::io::stdout())?;
    Ok(())
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
