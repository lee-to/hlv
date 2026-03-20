use std::io::{self, BufRead, Write};
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;

use super::style;
use crate::model::llm_map::{LlmMap, MapEntry, MapEntryKind};
use crate::model::policy::{ConstraintFile, ConstraintRule, PerformanceConstraints};
use crate::model::project::{ConstraintEntry, ProjectMap};

pub fn run_list(project_root: &Path, severity: Option<&str>, json: bool) -> Result<()> {
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;

    if json {
        return run_list_json(project_root, &project, severity);
    }

    style::header("constraints");

    let mut total_rules = 0u32;
    let mut by_severity = std::collections::HashMap::new();

    for entry in &project.constraints {
        let file_path = project_root.join(&entry.path);

        // Try as rule-based first
        if let Ok(cf) = ConstraintFile::load(&file_path) {
            let filtered: Vec<_> = cf
                .rules
                .iter()
                .filter(|r| severity.is_none_or(|s| r.severity == s))
                .collect();

            println!(
                "\n  {} {}  {}",
                "▶".blue(),
                entry.id.bold(),
                entry.path.dimmed()
            );

            for rule in &filtered {
                let sev_color = match rule.severity.as_str() {
                    "critical" => rule.severity.red(),
                    "high" => rule.severity.yellow(),
                    "medium" => rule.severity.normal(),
                    _ => rule.severity.dimmed(),
                };
                println!("    {} {:<30} {}", "├─".dimmed(), rule.id, sev_color);
                total_rules += 1;
                *by_severity.entry(rule.severity.clone()).or_insert(0u32) += 1;
            }
        } else if let Ok(pc) = PerformanceConstraints::load(&file_path) {
            // Metric-based
            println!(
                "\n  {} {}  {} {}",
                "▷".dimmed(),
                entry.id.bold(),
                entry.path.dimmed(),
                "(metric-based)".dimmed()
            );
            if let Some(ref d) = pc.defaults {
                let mut parts = vec![];
                if let Some(p95) = d.latency_p95_ms {
                    parts.push(format!("p95={}ms", p95));
                }
                if let Some(p99) = d.latency_p99_ms {
                    parts.push(format!("p99={}ms", p99));
                }
                if let Some(err) = d.error_rate_max_percent {
                    parts.push(format!("error_rate<{}%", err));
                }
                println!("    defaults: {}", parts.join(", ").dimmed());
            }
            println!("    overrides: {}", pc.overrides.len().to_string().dimmed());
        } else {
            println!(
                "\n  {} {}  {} {}",
                "?".yellow(),
                entry.id.bold(),
                entry.path.dimmed(),
                "(cannot parse)".red()
            );
        }
    }

    let constraint_count = project.constraints.len();
    let sev_summary: Vec<String> = ["critical", "high", "medium", "low"]
        .iter()
        .filter_map(|s| by_severity.get(*s).map(|count| format!("{} {}", count, s)))
        .collect();

    println!(
        "\n  {} constraints, {} rules{}",
        constraint_count.to_string().bold(),
        total_rules.to_string().bold(),
        if sev_summary.is_empty() {
            String::new()
        } else {
            format!(" ({})", sev_summary.join(", "))
        }
    );

    Ok(())
}

fn run_list_json(project_root: &Path, project: &ProjectMap, severity: Option<&str>) -> Result<()> {
    let mut result = vec![];

    for entry in &project.constraints {
        let file_path = project_root.join(&entry.path);

        if let Ok(cf) = ConstraintFile::load(&file_path) {
            let rules: Vec<serde_json::Value> = cf
                .rules
                .iter()
                .filter(|r| severity.is_none_or(|s| r.severity == s))
                .map(|r| {
                    let mut val = serde_json::json!({
                        "id": r.id,
                        "severity": r.severity,
                        "statement": r.statement,
                        "enforcement": r.enforcement,
                    });
                    if let Some(ref el) = r.error_level {
                        val["error_level"] = serde_json::json!(el);
                    }
                    val
                })
                .collect();

            result.push(serde_json::json!({
                "id": entry.id,
                "path": entry.path,
                "type": "rule-based",
                "owner": cf.owner,
                "version": cf.version,
                "rules": rules,
            }));
        } else if PerformanceConstraints::load(&file_path).is_ok() {
            result.push(serde_json::json!({
                "id": entry.id,
                "path": entry.path,
                "type": "metric-based",
            }));
        }
    }

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub fn run_show(project_root: &Path, name: &str, json: bool) -> Result<()> {
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;

    let entry = project
        .constraints
        .iter()
        .find(|c| c.id.contains(name))
        .ok_or_else(|| anyhow::anyhow!("Constraint matching '{}' not found", name))?;

    let file_path = project_root.join(&entry.path);

    if let Ok(cf) = ConstraintFile::load(&file_path) {
        if json {
            let val = serde_json::json!({
                "id": cf.id,
                "version": cf.version,
                "owner": cf.owner,
                "intent": cf.intent,
                "rules": cf.rules.iter().map(|r| {
                    let mut val = serde_json::json!({
                        "id": r.id,
                        "severity": r.severity,
                        "statement": r.statement,
                        "enforcement": r.enforcement,
                    });
                    if let Some(ref el) = r.error_level {
                        val["error_level"] = serde_json::json!(el);
                    }
                    val
                }).collect::<Vec<_>>(),
                "exceptions": cf.exceptions.as_ref().map(|e| serde_json::json!({
                    "process": e.process,
                    "max_exception_days": e.max_exception_days,
                })),
            });
            println!("{}", serde_json::to_string_pretty(&val)?);
        } else {
            style::header(&format!("constraints show {}", name));
            style::detail("ID", &cf.id);
            style::detail("Version", &cf.version);
            if let Some(ref owner) = cf.owner {
                style::detail("Owner", owner);
            }
            if let Some(ref intent) = cf.intent {
                style::detail("Intent", intent);
            }

            println!("\n  {}", "Rules".bold());
            for rule in &cf.rules {
                let sev_color = match rule.severity.as_str() {
                    "critical" => rule.severity.red(),
                    "high" => rule.severity.yellow(),
                    "medium" => rule.severity.normal(),
                    _ => rule.severity.dimmed(),
                };
                println!(
                    "    {} {} [{}]",
                    rule.id.bold(),
                    sev_color,
                    rule.enforcement.join(", ").dimmed()
                );
                println!("      {}", rule.statement);
            }

            if let Some(ref exc) = cf.exceptions {
                println!("\n  {}", "Exceptions".bold());
                if let Some(ref process) = exc.process {
                    style::detail("Process", process);
                }
                if let Some(days) = exc.max_exception_days {
                    style::detail("Max days", &days.to_string());
                }
            }

            println!("\n  {} rules", cf.rules.len().to_string().bold());
        }
    } else {
        anyhow::bail!(
            "Constraint '{}' is not rule-based or cannot be parsed",
            name
        );
    }

    Ok(())
}

pub fn run_add(
    project_root: &Path,
    name: &str,
    owner: Option<&str>,
    intent: Option<&str>,
    applies_to: &str,
) -> Result<()> {
    let project_path = project_root.join("project.yaml");
    let mut project = ProjectMap::load(&project_path)?;

    let constraint_id = format!("constraints.{}.{}", name, applies_to);
    let constraint_path = format!("human/constraints/{}.yaml", name);
    let abs_path = project_root.join(&constraint_path);

    if abs_path.exists() {
        anyhow::bail!("Constraint file already exists: {}", constraint_path);
    }

    // Create constraint file
    let cf = ConstraintFile {
        id: constraint_id.clone(),
        version: "1.0.0".to_string(),
        owner: owner.map(|s| s.to_string()),
        intent: intent.map(|s| s.to_string()),
        check_command: None,
        check_cwd: None,
        rules: vec![],
        exceptions: None,
    };

    // Ensure directory exists
    if let Some(parent) = abs_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    cf.save(&abs_path)?;

    // Add to project.yaml
    project.add_constraint(ConstraintEntry {
        id: constraint_id.clone(),
        path: constraint_path.clone(),
        applies_to: Some(applies_to.to_string()),
    })?;
    project.save(&project_path)?;

    // Add to llm/map.yaml if it exists
    if let Some(ref map_path) = project.paths.llm.map {
        let map_abs = project_root.join(map_path);
        if map_abs.exists() {
            let mut llm_map = LlmMap::load(&map_abs)?;
            let _ = llm_map.add_entry(MapEntry {
                path: constraint_path.clone(),
                kind: MapEntryKind::File,
                layer: "human".to_string(),
                description: format!("{} constraints", name),
            });
            llm_map.save(&map_abs)?;
        }
    }

    style::ok(&format!(
        "Created constraint '{}' at {}",
        constraint_id, constraint_path
    ));
    Ok(())
}

pub fn run_remove(project_root: &Path, name: &str, force: bool) -> Result<()> {
    let project_path = project_root.join("project.yaml");
    let mut project = ProjectMap::load(&project_path)?;

    let entry = project
        .constraints
        .iter()
        .find(|c| c.id.contains(name))
        .ok_or_else(|| anyhow::anyhow!("Constraint matching '{}' not found", name))?
        .clone();

    let file_path = project_root.join(&entry.path);

    // Count rules for confirmation
    let rule_count = ConstraintFile::load(&file_path)
        .map(|cf| cf.rules.len())
        .unwrap_or(0);

    if !force {
        print!(
            "  {} This will delete {} rules. Remove constraint '{}'? [y/N] ",
            "!".yellow().bold(),
            rule_count,
            name
        );
        io::stdout().flush()?;
        let stdin = io::stdin();
        let line = stdin.lock().lines().next().unwrap_or(Ok(String::new()))?;
        if !line.trim().eq_ignore_ascii_case("y") {
            style::hint("Cancelled");
            return Ok(());
        }
    }

    // Remove file
    if file_path.exists() {
        std::fs::remove_file(&file_path)?;
    }

    // Remove from project.yaml
    project.remove_constraint(&entry.id)?;
    project.save(&project_path)?;

    // Remove from llm/map.yaml if it exists
    if let Some(ref map_path) = project.paths.llm.map {
        let map_abs = project_root.join(map_path);
        if map_abs.exists() {
            let mut llm_map = LlmMap::load(&map_abs)?;
            let _ = llm_map.remove_entry(&entry.path);
            llm_map.save(&map_abs)?;
        }
    }

    style::ok(&format!("Removed constraint '{}'", entry.id));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_add_rule(
    project_root: &Path,
    constraint_name: &str,
    rule_id: &str,
    severity: &str,
    statement: &str,
    check_command: Option<&str>,
    check_cwd: Option<&str>,
    error_level: Option<&str>,
) -> Result<()> {
    // Validate severity
    if !["critical", "high", "medium", "low"].contains(&severity) {
        anyhow::bail!(
            "Invalid severity '{}'. Must be: critical, high, medium, low",
            severity
        );
    }

    // Validate error_level if provided
    if let Some(el) = error_level {
        if !["error", "warning", "info"].contains(&el) {
            anyhow::bail!(
                "Invalid error_level '{}'. Must be: error, warning, info",
                el
            );
        }
    }

    let project = ProjectMap::load(&project_root.join("project.yaml"))?;

    let entry = project
        .constraints
        .iter()
        .find(|c| c.id.contains(constraint_name))
        .ok_or_else(|| anyhow::anyhow!("Constraint matching '{}' not found", constraint_name))?;

    let file_path = project_root.join(&entry.path);

    // Ensure it's rule-based (not performance)
    let mut cf = ConstraintFile::load(&file_path)
        .context("Cannot load as rule-based constraint (may be metric-based)")?;

    cf.add_rule(ConstraintRule {
        id: rule_id.to_string(),
        severity: severity.to_string(),
        statement: statement.to_string(),
        enforcement: vec![],
        check_command: check_command.map(|s| s.to_string()),
        check_cwd: check_cwd.map(|s| s.to_string()),
        error_level: error_level.map(|s| s.to_string()),
    })?;

    cf.save(&file_path)?;

    style::ok(&format!(
        "Added rule '{}' ({}) to '{}'",
        rule_id, severity, constraint_name
    ));
    Ok(())
}

pub fn run_remove_rule(project_root: &Path, constraint_name: &str, rule_id: &str) -> Result<()> {
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;

    let entry = project
        .constraints
        .iter()
        .find(|c| c.id.contains(constraint_name))
        .ok_or_else(|| anyhow::anyhow!("Constraint matching '{}' not found", constraint_name))?;

    let file_path = project_root.join(&entry.path);

    let mut cf = ConstraintFile::load(&file_path)
        .context("Cannot load as rule-based constraint (may be metric-based)")?;

    cf.remove_rule(rule_id)?;
    cf.save(&file_path)?;

    style::ok(&format!(
        "Removed rule '{}' from '{}'",
        rule_id, constraint_name
    ));
    Ok(())
}

pub fn run_check(
    project_root: &Path,
    constraint: Option<&str>,
    rule: Option<&str>,
    json: bool,
) -> Result<()> {
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;

    let (mut diags, mut results) =
        crate::check::constraints::run_constraint_checks(project_root, &project, constraint, rule);

    // Also run file-level checks (CST-060)
    if rule.is_none() {
        let (file_diags, file_results) =
            crate::check::constraints::run_file_level_checks(project_root, &project, constraint);
        diags.extend(file_diags);
        results.extend(file_results);
    }

    if json {
        let output = serde_json::json!({
            "results": results,
            "diagnostics": diags.iter().map(|d| serde_json::json!({
                "code": d.code,
                "severity": format!("{:?}", d.severity).to_lowercase(),
                "message": d.message,
                "file": d.file,
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    style::header("constraints check");

    if results.is_empty() {
        style::hint("No rules with check_command found");
        return Ok(());
    }

    let mut passed = 0u32;
    let mut failed = 0u32;

    for r in &results {
        let icon = if r.passed {
            "✓".green().to_string()
        } else {
            "✗".red().to_string()
        };
        let sev_color = match r.severity.as_str() {
            "critical" => r.severity.red(),
            "high" => r.severity.yellow(),
            "medium" => r.severity.normal(),
            _ => r.severity.dimmed(),
        };
        println!(
            "  {} {} {} [{}] {}",
            icon,
            r.rule_id.bold(),
            sev_color,
            r.constraint_id.dimmed(),
            if r.passed {
                "ok".dimmed().to_string()
            } else {
                r.message.clone()
            }
        );
        if r.passed {
            passed += 1;
        } else {
            failed += 1;
        }
    }

    let failed_str = if failed > 0 {
        failed.to_string().red().bold().to_string()
    } else {
        failed.to_string().dimmed().to_string()
    };
    println!(
        "\n  {} passed, {} failed",
        passed.to_string().green().bold(),
        failed_str
    );

    // Print diagnostics
    for d in &diags {
        d.print();
    }

    Ok(())
}

/// Get constraint check results as structured data (for MCP / JSON consumers).
pub fn get_constraint_check_results(
    project_root: &Path,
    constraint: Option<&str>,
    rule: Option<&str>,
) -> Result<serde_json::Value> {
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;

    let (mut diags, mut results) =
        crate::check::constraints::run_constraint_checks(project_root, &project, constraint, rule);

    // Also run file-level checks (CST-060)
    if rule.is_none() {
        let (file_diags, file_results) =
            crate::check::constraints::run_file_level_checks(project_root, &project, constraint);
        diags.extend(file_diags);
        results.extend(file_results);
    }

    Ok(serde_json::json!({
        "results": results,
        "diagnostics": diags.iter().map(|d| serde_json::json!({
            "code": d.code,
            "severity": format!("{:?}", d.severity).to_lowercase(),
            "message": d.message,
            "file": d.file,
        })).collect::<Vec<_>>(),
    }))
}
