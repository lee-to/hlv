use std::path::{Component, Path};

use anyhow::Result;
use colored::Colorize;

use super::style;
use crate::check::{Diagnostic, Severity};
use crate::model::policy::{ConstraintFile, GatesPolicy};
use crate::model::project::ProjectMap;
use crate::util::command_parser::parse_portable_command;
use crate::util::display_width::{display_width, pad_display_width, truncate_display_width};

#[derive(Debug, Clone, serde::Serialize)]
pub struct DoctorReport {
    pub diagnostics: Vec<Diagnostic>,
    pub fixed: Vec<String>,
    pub exit_code: i32,
}

pub fn run(root: &Path, json: bool, fix: bool) -> Result<()> {
    let report = doctor_report(root, fix)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        std::process::exit(report.exit_code);
    }

    style::header("doctor");
    if report.diagnostics.is_empty() {
        style::ok("environment checks passed");
    } else {
        for diag in &report.diagnostics {
            diag.print();
        }
    }
    if !report.fixed.is_empty() {
        style::section("Fixed");
        for path in &report.fixed {
            println!("    {} {}", "+".green(), path);
        }
    }
    if report.exit_code != 0 {
        std::process::exit(report.exit_code);
    }
    Ok(())
}

pub fn doctor_report(root: &Path, fix: bool) -> Result<DoctorReport> {
    let mut diagnostics = Vec::new();
    let mut fixed = Vec::new();
    // Config artifacts live under the config root (`.hlv/` for adopted
    // projects); gate/constraint command cwds resolve against the repo root.
    let config_root = &crate::config_root(root);
    let project_path = config_root.join("project.yaml");
    if !project_path.exists() {
        diagnostics
            .push(Diagnostic::error("DOC-001", "project.yaml not found").with_file("project.yaml"));
        return Ok(report(diagnostics, fixed));
    }

    let project = match ProjectMap::load(&project_path) {
        Ok(project) => project,
        Err(e) => {
            diagnostics.push(
                Diagnostic::error("DOC-002", format!("Cannot parse project.yaml: {e}"))
                    .with_file("project.yaml"),
            );
            return Ok(report(diagnostics, fixed));
        }
    };

    check_schema_compat(config_root, &project, &mut diagnostics);
    check_project_paths(config_root, &project, fix, &mut diagnostics, &mut fixed)?;
    check_llm_paths(&project, &mut diagnostics);
    check_gate_commands(root, config_root, &project, &mut diagnostics);
    check_constraint_commands(root, config_root, &project, &mut diagnostics);
    check_non_ascii_smoke(&mut diagnostics);

    Ok(report(diagnostics, fixed))
}

fn report(diagnostics: Vec<Diagnostic>, fixed: Vec<String>) -> DoctorReport {
    let exit_code = if diagnostics
        .iter()
        .any(|d| matches!(d.severity, Severity::Error))
    {
        1
    } else {
        0
    };
    DoctorReport {
        diagnostics,
        fixed,
        exit_code,
    }
}

fn check_schema_compat(root: &Path, project: &ProjectMap, diagnostics: &mut Vec<Diagnostic>) {
    if project.schema_version != 1 {
        diagnostics.push(
            Diagnostic::error(
                "DOC-080",
                format!(
                    "Unsupported project schema_version {}",
                    project.schema_version
                ),
            )
            .with_file("project.yaml"),
        );
    }
    let schema = root.join("schema/project-schema.json");
    if !schema.exists() {
        diagnostics.push(
            Diagnostic::warning("DOC-081", "schema/project-schema.json not found")
                .with_file("schema/project-schema.json"),
        );
    }
    if let Some(spec) = project.spec.as_deref() {
        if !root.join(spec).exists() {
            diagnostics.push(
                Diagnostic::warning("DOC-082", format!("project.spec path not found: {spec}"))
                    .with_file("project.yaml"),
            );
        }
    }
}

fn check_project_paths(
    root: &Path,
    project: &ProjectMap,
    fix: bool,
    diagnostics: &mut Vec<Diagnostic>,
    fixed: &mut Vec<String>,
) -> Result<()> {
    ensure_file_parent(root, &project.paths.human.glossary, fix, diagnostics, fixed)?;
    ensure_dir(
        root,
        &project.paths.human.constraints,
        fix,
        diagnostics,
        fixed,
    )?;
    if let Some(path) = project.paths.human.artifacts.as_deref() {
        ensure_dir(root, path, fix, diagnostics, fixed)?;
    }
    ensure_dir(
        root,
        &project.paths.validation.scenarios,
        fix,
        diagnostics,
        fixed,
    )?;
    if let Some(path) = project.paths.validation.test_specs.as_deref() {
        ensure_dir(root, path, fix, diagnostics, fixed)?;
    }
    ensure_file_parent(
        root,
        &project.paths.validation.gates_policy,
        fix,
        diagnostics,
        fixed,
    )?;
    if let Some(path) = project.paths.validation.traceability.as_deref() {
        ensure_file_parent(root, path, fix, diagnostics, fixed)?;
    }
    if let Some(path) = project.paths.validation.verify_report.as_deref() {
        ensure_file_parent(root, path, fix, diagnostics, fixed)?;
    }
    if let Some(path) = project.paths.llm.src.as_deref() {
        ensure_dir(root, path, fix, diagnostics, fixed)?;
    }
    if let Some(path) = project.paths.llm.tests.as_deref() {
        ensure_dir(root, path, fix, diagnostics, fixed)?;
    }
    if let Some(path) = project.paths.llm.map.as_deref() {
        ensure_file_parent(root, path, fix, diagnostics, fixed)?;
    }
    Ok(())
}

fn ensure_dir(
    root: &Path,
    rel: &str,
    fix: bool,
    diagnostics: &mut Vec<Diagnostic>,
    fixed: &mut Vec<String>,
) -> Result<()> {
    if !is_safe_relative_path(rel) {
        diagnostics.push(
            Diagnostic::error("DOC-020", format!("Path must be project-relative: {rel}"))
                .with_file("project.yaml"),
        );
        return Ok(());
    }
    let path = root.join(rel);
    if path.is_dir() {
        return Ok(());
    }
    if fix {
        std::fs::create_dir_all(&path)?;
        fixed.push(normalize_dir(rel));
    } else {
        diagnostics.push(
            Diagnostic::error("DOC-021", format!("Directory missing: {rel}"))
                .with_file("project.yaml"),
        );
    }
    Ok(())
}

fn ensure_file_parent(
    root: &Path,
    rel: &str,
    fix: bool,
    diagnostics: &mut Vec<Diagnostic>,
    fixed: &mut Vec<String>,
) -> Result<()> {
    if !is_safe_relative_path(rel) {
        diagnostics.push(
            Diagnostic::error("DOC-020", format!("Path must be project-relative: {rel}"))
                .with_file("project.yaml"),
        );
        return Ok(());
    }
    let path = root.join(rel);
    if path.exists() {
        return Ok(());
    }
    if let Some(parent_rel) = Path::new(rel).parent().and_then(Path::to_str) {
        if !parent_rel.is_empty() && !root.join(parent_rel).is_dir() {
            if fix {
                std::fs::create_dir_all(root.join(parent_rel))?;
                fixed.push(normalize_dir(parent_rel));
            } else {
                diagnostics.push(
                    Diagnostic::error(
                        "DOC-022",
                        format!("Parent directory missing for file path: {rel}"),
                    )
                    .with_file("project.yaml"),
                );
            }
        }
    }
    Ok(())
}

fn check_llm_paths(project: &ProjectMap, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(src) = project.paths.llm.src.as_deref() {
        if !normalize_path(src).starts_with("llm/") {
            diagnostics.push(
                Diagnostic::error("DOC-030", format!("paths.llm.src is outside llm/: {src}"))
                    .with_file("project.yaml"),
            );
        }
    }
    if let Some(tests) = project.paths.llm.tests.as_deref() {
        if !normalize_path(tests).starts_with("llm/") {
            diagnostics.push(
                Diagnostic::error(
                    "DOC-031",
                    format!("paths.llm.tests is outside llm/: {tests}"),
                )
                .with_file("project.yaml"),
            );
        }
    }
    if project.paths.llm.map.as_deref().is_some_and(str::is_empty) {
        diagnostics.push(
            Diagnostic::error("DOC-032", "paths.llm.map must not be empty")
                .with_file("project.yaml"),
        );
    }
}

fn check_gate_commands(
    root: &Path,
    config_root: &Path,
    project: &ProjectMap,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = config_root.join(&project.paths.validation.gates_policy);
    let Ok(policy) = GatesPolicy::load(&path) else {
        return;
    };
    for gate in &policy.gates {
        if let Some(command) = gate.command.as_deref() {
            if let Err(e) = parse_portable_command(command) {
                diagnostics.push(
                    Diagnostic::error(
                        "DOC-040",
                        format!("Gate '{}' command is not portable: {:?}", gate.id, e),
                    )
                    .with_file(&project.paths.validation.gates_policy),
                );
            }
        }
        if let Some(cwd) = gate.cwd.as_deref() {
            if !root.join(cwd).is_dir() {
                diagnostics.push(
                    Diagnostic::error(
                        "DOC-041",
                        format!("Gate '{}' cwd does not exist: {cwd}", gate.id),
                    )
                    .with_file(&project.paths.validation.gates_policy),
                );
            }
        }
    }
}

fn check_constraint_commands(
    root: &Path,
    config_root: &Path,
    project: &ProjectMap,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for entry in &project.constraints {
        let Ok(file) = ConstraintFile::load(&config_root.join(&entry.path)) else {
            continue;
        };
        if let Some(command) = file.check_command.as_deref() {
            check_command(command, &entry.path, "constraint file", diagnostics);
        }
        if let Some(cwd) = file.check_cwd.as_deref() {
            check_cwd(root, cwd, &entry.path, diagnostics);
        }
        for rule in &file.rules {
            if let Some(command) = rule.check_command.as_deref() {
                check_command(command, &entry.path, &rule.id, diagnostics);
            }
            if let Some(cwd) = rule.check_cwd.as_deref() {
                check_cwd(root, cwd, &entry.path, diagnostics);
            }
        }
    }
}

fn check_command(command: &str, file: &str, label: &str, diagnostics: &mut Vec<Diagnostic>) {
    if let Err(e) = parse_portable_command(command) {
        diagnostics.push(
            Diagnostic::error(
                "DOC-040",
                format!("{label} command is not portable: {:?}", e),
            )
            .with_file(file),
        );
    }
}

fn check_cwd(root: &Path, cwd: &str, file: &str, diagnostics: &mut Vec<Diagnostic>) {
    if !root.join(cwd).is_dir() {
        diagnostics.push(
            Diagnostic::error("DOC-041", format!("command cwd does not exist: {cwd}"))
                .with_file(file),
        );
    }
}

fn check_non_ascii_smoke(diagnostics: &mut Vec<Diagnostic>) {
    let sample = "Проверка 語 e\u{301}";
    let padded = pad_display_width(sample, 24);
    let truncated = truncate_display_width(&padded, 16);
    if display_width(&truncated) > 16 {
        diagnostics.push(Diagnostic::error(
            "DOC-070",
            "display-width smoke test exceeded target width",
        ));
    }
}

fn is_safe_relative_path(path: &str) -> bool {
    let path = Path::new(path);
    path.is_relative()
        && !path.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        })
}

fn normalize_path(path: &str) -> String {
    path.trim_start_matches("./")
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string()
        + "/"
}

fn normalize_dir(path: &str) -> String {
    let normalized = path.replace('\\', "/").trim_end_matches('/').to_string();
    format!("{normalized}/")
}
