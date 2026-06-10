use std::path::Path;
use std::sync::mpsc;

use anyhow::Result;
use colored::Colorize;

use super::style;
use crate::check::{self, Diagnostic, Severity};
use crate::model::contract_md::ContractMd;
use crate::model::contract_yaml::ContractYaml;
use crate::model::glossary::Glossary;
use crate::model::milestone::MilestoneMap;
use crate::model::policy::GatesPolicy;
use crate::model::project::{ContractEntry, ContractStatus, ProjectMap, Strictness};
use crate::model::waiver::{Waiver, WaiverFile};

#[derive(Debug, Clone, Copy, Default)]
pub struct CheckOptions {
    pub strict: bool,
    pub with_waivers: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckReport {
    pub diagnostics: Vec<Diagnostic>,
    pub waived: Vec<WaivedDiagnostic>,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub exit_code: i32,
    pub strictness: Strictness,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WaivedDiagnostic {
    pub diagnostic: Diagnostic,
    pub waiver: Waiver,
}

pub fn run(
    project_root: &Path,
    watch: bool,
    json: bool,
    strict: bool,
    with_waivers: bool,
) -> Result<()> {
    let options = CheckOptions {
        strict,
        with_waivers,
    };
    if json {
        let report = get_check_report(project_root, options)?;
        let output = serde_json::json!({
            "diagnostics": report.diagnostics,
            "waived": report.waived,
            "errors": report.errors,
            "warnings": report.warnings,
            "infos": report.infos,
            "strictness": report.strictness,
            "exit_code": report.exit_code,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        std::process::exit(report.exit_code);
    }

    let report = get_check_report(project_root, options)?;
    print_check_report(&report);
    let mut code = report.exit_code;

    if code == 0 && report.strictness != Strictness::Relaxed {
        let (_, gate_failures, _) = super::gates::run_gate_commands(project_root, None)?;
        if gate_failures > 0 {
            code = 1;
        }
    }

    if watch {
        style::hint("Watching for changes... (Ctrl+C to stop)");
        println!();
        watch_loop(project_root, options)?;
    } else {
        std::process::exit(code);
    }

    Ok(())
}

/// Get all diagnostics without printing — for JSON output
pub fn get_check_diagnostics(root: &Path) -> Result<(Vec<Diagnostic>, i32)> {
    let report = get_check_report(root, CheckOptions::default())?;
    Ok((report.diagnostics, report.exit_code))
}

pub fn get_check_report(root: &Path, options: CheckOptions) -> Result<CheckReport> {
    let strictness = effective_strictness(root, options.strict);
    let mut all_diags = collect_diagnostics(root, &strictness)?;

    if strictness == Strictness::Strict {
        promote_warnings_to_errors(&mut all_diags);
    }

    let mut waived = Vec::new();
    if options.with_waivers {
        let mut waiver_diags = apply_waivers(root, &mut all_diags, &mut waived);
        if strictness == Strictness::Strict {
            promote_warnings_to_errors(&mut waiver_diags);
        }
        all_diags.extend(waiver_diags);
    }

    let errors = all_diags
        .iter()
        .filter(|d| matches!(d.severity, Severity::Error))
        .count();
    let warnings = all_diags
        .iter()
        .filter(|d| matches!(d.severity, Severity::Warning))
        .count();
    let infos = all_diags
        .iter()
        .filter(|d| matches!(d.severity, Severity::Info))
        .count();
    let exit_code = check::exit_code(&all_diags);

    Ok(CheckReport {
        diagnostics: all_diags,
        waived,
        errors,
        warnings,
        infos,
        exit_code,
        strictness,
    })
}

fn collect_diagnostics(root: &Path, strictness: &Strictness) -> Result<Vec<Diagnostic>> {
    let mut all_diags: Vec<Diagnostic> = Vec::new();

    let project_diags = check::project_map::check_project_map(root);
    let has_prj_fatal = project_diags.iter().any(|d| d.code == "PRJ-001");
    all_diags.extend(project_diags);

    if has_prj_fatal {
        return Ok(all_diags);
    }

    let project = ProjectMap::load(&root.join("project.yaml"))?;
    let glossary_path = root.join(&project.paths.human.glossary);
    let glossary = match Glossary::load(&glossary_path) {
        Ok(glossary) => glossary,
        Err(e) => {
            if glossary_path.exists() {
                all_diags.push(
                    Diagnostic::error("GLO-001", format!("Cannot parse glossary: {e}"))
                        .with_file(&project.paths.human.glossary),
                );
            }
            Glossary {
                schema_version: None,
                domain: None,
                types: Default::default(),
                enums: Default::default(),
                terms: Default::default(),
                rules: Vec::new(),
            }
        }
    };

    let (milestone_info, mst_diags) = load_milestone_info(root);
    all_diags.extend(mst_diags);

    let (contracts, trace_path_str, _) = match &milestone_info {
        Some((milestones, milestone_id)) => {
            let ms_contracts = collect_milestone_contracts(root, milestone_id);
            let ms_trace = format!("human/milestones/{}/traceability.yaml", milestone_id);
            let stage_label = milestones
                .current
                .as_ref()
                .and_then(|c| {
                    c.stage
                        .and_then(|sid| c.stages.iter().find(|s| s.id == sid))
                })
                .map(|s| format!("stage {} ({})", s.id, s.status))
                .unwrap_or_else(|| "milestone active".to_string());
            (ms_contracts, ms_trace, stage_label)
        }
        None => {
            let trace = project
                .paths
                .validation
                .traceability
                .clone()
                .unwrap_or_else(|| "validation/traceability.yaml".to_string());
            (Vec::new(), trace, project.status.to_string())
        }
    };

    all_diags.extend(check::contracts::check_contracts(
        root, &contracts, &glossary,
    ));

    all_diags.extend(check::validation::check_test_specs(root, &contracts));

    if root.join(&trace_path_str).exists() {
        all_diags.extend(check::traceability::check_traceability(
            root,
            &trace_path_str,
            &contracts,
        ));
    }

    if let Some((_, milestone_id)) = &milestone_info {
        all_diags.extend(check::plan::check_stage_plans(
            root,
            milestone_id,
            &contracts,
        ));
    }

    if let Some(ref stack) = project.stack {
        all_diags.extend(check::stack::check_stack(stack));
    }

    all_diags.extend(check::artifacts::check_artifacts(root, &project));

    {
        let tests_path = project.paths.llm.tests.as_deref();
        all_diags.extend(check::code_trace::check_code_trace(
            root,
            &contracts,
            &project.constraints,
            &project.paths.llm.src,
            tests_path,
            project.features.hlv_markers,
        ));
    }

    all_diags.extend(check::sec_markers::check_sec_markers(
        root,
        &project.paths.llm.src,
        project.features.security_markers,
    ));

    if let Some(ref map_path) = project.paths.llm.map {
        all_diags.extend(check::llm_map::check_llm_map(
            root,
            map_path,
            &project.paths.llm,
        ));
    }

    if !project.constraints.is_empty() {
        all_diags.extend(check::constraints::check_constraints(root, &project));
        if strictness != &Strictness::Relaxed {
            // CST-050: run rule-level check_commands
            let (cst050, _) = check::constraints::run_constraint_checks(root, &project, None, None);
            all_diags.extend(cst050);
            // CST-060: run file-level check_commands
            let (cst060, _) = check::constraints::run_file_level_checks(root, &project, None);
            all_diags.extend(cst060);
        }
    }

    {
        let gates_path = root.join(&project.paths.validation.gates_policy);
        if gates_path.exists() {
            if let Err(e) = GatesPolicy::load(&gates_path) {
                all_diags.push(
                    Diagnostic::error("GAT-001", format!("Cannot parse gates-policy.yaml: {}", e))
                        .with_file(&project.paths.validation.gates_policy),
                );
            }
        }
    }

    // Task diagnostics (TSK-010..050)
    all_diags.extend(check::tasks::check_tasks(root));

    // Phase-aware downgrade
    if strictness != &Strictness::Strict {
        if let Some((milestones, _)) = &milestone_info {
            if let Some(stage_status) = milestones
                .current
                .as_ref()
                .and_then(|c| {
                    c.stage
                        .and_then(|sid| c.stages.iter().find(|s| s.id == sid))
                })
                .map(|s| &s.status)
            {
                check::apply_phase_expectations_stage(&mut all_diags, stage_status);
            } else {
                check::apply_phase_expectations(&mut all_diags, &project.status);
            }
        } else {
            check::apply_phase_expectations(&mut all_diags, &project.status);
        }
    }

    Ok(all_diags)
}

fn effective_strictness(root: &Path, strict: bool) -> Strictness {
    if strict {
        return Strictness::Strict;
    }
    ProjectMap::load(&root.join("project.yaml"))
        .ok()
        .and_then(|project| project.validation.map(|validation| validation.strictness))
        .unwrap_or_default()
}

fn promote_warnings_to_errors(diags: &mut [Diagnostic]) {
    for diag in diags {
        if matches!(diag.severity, Severity::Warning) {
            diag.severity = Severity::Error;
            diag.message = format!("{} (strict mode)", diag.message);
        }
    }
}

fn apply_waivers(
    root: &Path,
    diags: &mut Vec<Diagnostic>,
    waived: &mut Vec<WaivedDiagnostic>,
) -> Vec<Diagnostic> {
    let waiver_path = root.join("validation/waivers.yaml");
    if !waiver_path.exists() {
        return Vec::new();
    }

    let file = match WaiverFile::load(&waiver_path) {
        Ok(file) => file,
        Err(e) => {
            return vec![Diagnostic::error(
                "WVR-001",
                format!("Cannot parse validation/waivers.yaml: {e}"),
            )
            .with_file("validation/waivers.yaml")];
        }
    };

    let mut waiver_diags = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let today = chrono::Local::now().date_naive();

    for waiver in file.waivers {
        let key = (waiver.code.to_ascii_uppercase(), waiver.file.clone());
        if !seen.insert(key) {
            waiver_diags.push(
                Diagnostic::warning(
                    "WVR-010",
                    format!("Duplicate waiver for {} in {}", waiver.code, waiver.file),
                )
                .with_file("validation/waivers.yaml"),
            );
            continue;
        }

        if waiver.reason.trim().is_empty() {
            waiver_diags.push(
                Diagnostic::error(
                    "WVR-011",
                    format!(
                        "Waiver for {} in {} has empty reason",
                        waiver.code, waiver.file
                    ),
                )
                .with_file("validation/waivers.yaml"),
            );
            continue;
        }

        if waiver.expires < today {
            waiver_diags.push(
                Diagnostic::warning(
                    "WVR-020",
                    format!(
                        "Expired waiver for {} in {} expired on {}",
                        waiver.code, waiver.file, waiver.expires
                    ),
                )
                .with_file("validation/waivers.yaml"),
            );
            continue;
        }

        if let Some(pos) = diags.iter().position(|diag| {
            diag.code.eq_ignore_ascii_case(&waiver.code)
                && diag.file.as_deref() == Some(waiver.file.as_str())
        }) {
            let diagnostic = diags.remove(pos);
            waived.push(WaivedDiagnostic { diagnostic, waiver });
        } else {
            waiver_diags.push(
                Diagnostic::warning(
                    "WVR-030",
                    format!(
                        "Waiver for {} in {} did not match any diagnostic",
                        waiver.code, waiver.file
                    ),
                )
                .with_file("validation/waivers.yaml"),
            );
        }
    }

    waiver_diags
}

fn print_check_report(report: &CheckReport) {
    style::header("check");
    style::detail("strictness", &report.strictness.to_string());
    style::section("Diagnostics");
    if report.diagnostics.is_empty() {
        style::ok("all checks passed");
    } else {
        for diag in &report.diagnostics {
            diag.print();
        }
    }

    if !report.waived.is_empty() {
        style::section("Waived diagnostics");
        for item in &report.waived {
            let file = item.diagnostic.file.as_deref().unwrap_or("-");
            println!(
                "    {} [{}] {} {} ({}, expires {})",
                "·".dimmed(),
                item.diagnostic.code.dimmed(),
                item.diagnostic.message,
                file.dimmed(),
                item.waiver.reason.dimmed(),
                item.waiver.expires
            );
        }
    }

    style::separator();
    let status = if report.exit_code == 0 && report.warnings > 0 {
        "PASSED".yellow().bold()
    } else if report.exit_code == 0 {
        "PASSED".green().bold()
    } else {
        "FAILED".red().bold()
    };
    println!(
        "\n  {} — {} error(s), {} warning(s), {} info",
        status, report.errors, report.warnings, report.infos
    );
    println!();
}

/// Load milestone info if milestones.yaml exists with a current milestone.
fn load_milestone_info(root: &Path) -> (Option<(MilestoneMap, String)>, Vec<Diagnostic>) {
    let path = root.join("milestones.yaml");
    if !path.exists() {
        return (None, Vec::new());
    }
    match MilestoneMap::load(&path) {
        Ok(milestones) => {
            let id = match milestones.current.as_ref() {
                Some(c) => c.id.clone(),
                None => return (None, Vec::new()),
            };
            (Some((milestones, id)), Vec::new())
        }
        Err(e) => {
            let diag = Diagnostic::error("MST-001", format!("Cannot parse milestones.yaml: {}", e))
                .with_file("milestones.yaml");
            (None, vec![diag])
        }
    }
}

/// Build ContractEntry list by scanning human/milestones/<id>/contracts/ directory.
fn collect_milestone_contracts(root: &Path, milestone_id: &str) -> Vec<ContractEntry> {
    let contracts_dir = root
        .join("human/milestones")
        .join(milestone_id)
        .join("contracts");
    if !contracts_dir.is_dir() {
        return Vec::new();
    }

    let test_specs_dir = format!("human/milestones/{}/test-specs", milestone_id);
    let contracts_rel = format!("human/milestones/{}/contracts", milestone_id);

    let mut entries = Vec::new();
    let mut seen = std::collections::HashSet::new();

    if let Ok(dir) = std::fs::read_dir(&contracts_dir) {
        for file in dir.flatten() {
            let name = file.file_name().to_string_lossy().to_string();
            // Extract contract id: order.create.md → order.create
            let contract_id = if let Some(base) = name.strip_suffix(".md") {
                base.to_string()
            } else if let Some(base) = name.strip_suffix(".yaml") {
                base.to_string()
            } else {
                continue;
            };

            if !seen.insert(contract_id.clone()) {
                continue;
            }

            let md_path = format!("{}/{}.md", contracts_rel, contract_id);
            let yaml_path = format!("{}/{}.yaml", contracts_rel, contract_id);
            let test_spec = format!("{}/{}.md", test_specs_dir, contract_id);
            let version = collect_contract_version(root, &md_path, &yaml_path);

            entries.push(ContractEntry {
                id: contract_id,
                version,
                path: md_path,
                yaml_path: if root.join(&yaml_path).exists() {
                    Some(yaml_path)
                } else {
                    None
                },
                owner: None,
                status: ContractStatus::Generated,
                test_spec: if root.join(&test_spec).exists() {
                    Some(test_spec)
                } else {
                    None
                },
                depends_on: Vec::new(),
                artifacts: Vec::new(),
            });
        }
    }

    entries
}

fn collect_contract_version(root: &Path, md_path: &str, yaml_path: &str) -> String {
    let yaml_full_path = root.join(yaml_path);
    if let Ok(contract) = ContractYaml::load(&yaml_full_path) {
        return contract.version;
    }

    let md_full_path = root.join(md_path);
    if let Ok(text) = std::fs::read_to_string(md_full_path) {
        let contract = ContractMd::from_markdown(&text);
        if !contract.version.is_empty() {
            return contract.version;
        }
    }

    "1.0.0".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_contract_version_prefers_yaml_then_md_then_default() {
        let tmp = tempfile::tempdir().unwrap();
        let contracts_dir = tmp.path().join("human/milestones/001/contracts");
        std::fs::create_dir_all(&contracts_dir).unwrap();

        std::fs::write(
            contracts_dir.join("order.create.md"),
            "# order.create v2.0.0\n",
        )
        .unwrap();
        std::fs::write(
            contracts_dir.join("order.create.yaml"),
            "id: order.create\nversion: 2.1.0\n",
        )
        .unwrap();

        assert_eq!(
            collect_contract_version(
                tmp.path(),
                "human/milestones/001/contracts/order.create.md",
                "human/milestones/001/contracts/order.create.yaml",
            ),
            "2.1.0"
        );

        std::fs::remove_file(contracts_dir.join("order.create.yaml")).unwrap();
        assert_eq!(
            collect_contract_version(
                tmp.path(),
                "human/milestones/001/contracts/order.create.md",
                "human/milestones/001/contracts/order.create.yaml",
            ),
            "2.0.0"
        );

        assert_eq!(
            collect_contract_version(tmp.path(), "missing.md", "missing.yaml"),
            "1.0.0"
        );
    }
}

fn watch_loop(root: &Path, options: CheckOptions) -> Result<()> {
    use notify::{RecursiveMode, Watcher};

    let (tx, rx) = mpsc::channel();

    let mut watcher =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                    let _ = tx.send(());
                }
            }
        })?;

    let watch_paths = ["human", "validation", "project.yaml"];
    for p in &watch_paths {
        let full = root.join(p);
        if full.exists() {
            let mode = if full.is_dir() {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };
            watcher.watch(&full, mode)?;
        }
    }

    loop {
        rx.recv()?;
        while rx.try_recv().is_ok() {}
        std::thread::sleep(std::time::Duration::from_millis(200));
        while rx.try_recv().is_ok() {}

        let now = chrono::Local::now().format("%H:%M:%S");
        println!(
            "\n  {} [{}] Change detected, re-checking...",
            "↻".blue(),
            now
        );
        if let Err(e) = get_check_report(root, options).map(|report| print_check_report(&report)) {
            style::fatal(&style::format_error(&e));
        }
    }
}
