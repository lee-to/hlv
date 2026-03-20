use std::path::Path;
use std::sync::mpsc;

use anyhow::Result;
use colored::Colorize;

use super::style;
use crate::check::{self, Diagnostic};
use crate::model::glossary::Glossary;
use crate::model::milestone::MilestoneMap;
use crate::model::policy::GatesPolicy;
use crate::model::project::{ContractEntry, ContractStatus, ProjectMap};

pub fn run(project_root: &Path, watch: bool, json: bool) -> Result<()> {
    if json {
        let (diags, code) = get_check_diagnostics(project_root)?;
        let output = serde_json::json!({
            "diagnostics": diags.iter().map(|d| serde_json::json!({
                "code": d.code,
                "severity": format!("{:?}", d.severity).to_lowercase(),
                "message": d.message,
                "file": d.file,
            })).collect::<Vec<_>>(),
            "errors": diags.iter().filter(|d| matches!(d.severity, check::Severity::Error)).count(),
            "warnings": diags.iter().filter(|d| matches!(d.severity, check::Severity::Warning)).count(),
            "exit_code": code,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        std::process::exit(code);
    }

    let code = run_checks(project_root)?;

    if watch {
        style::hint("Watching for changes... (Ctrl+C to stop)");
        println!();
        watch_loop(project_root)?;
    } else {
        std::process::exit(code);
    }

    Ok(())
}

/// Get all diagnostics without printing — for JSON output
pub fn get_check_diagnostics(root: &Path) -> Result<(Vec<Diagnostic>, i32)> {
    let mut all_diags: Vec<Diagnostic> = Vec::new();

    let project_diags = check::project_map::check_project_map(root);
    let has_prj_fatal = project_diags.iter().any(|d| d.code == "PRJ-001");
    all_diags.extend(project_diags);

    if has_prj_fatal {
        let code = check::exit_code(&all_diags);
        return Ok((all_diags, code));
    }

    let project = ProjectMap::load(&root.join("project.yaml"))?;
    let glossary_path = root.join(&project.paths.human.glossary);
    let glossary = Glossary::load(&glossary_path).unwrap_or_else(|_| Glossary {
        schema_version: None,
        domain: None,
        types: Default::default(),
        enums: Default::default(),
        terms: Default::default(),
        rules: Vec::new(),
    });

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

    {
        let tests_path = project.paths.llm.tests.as_deref();
        all_diags.extend(check::code_trace::check_code_trace(
            root,
            &contracts,
            &project.constraints,
            &project.paths.llm.src,
            tests_path,
        ));
    }

    if let Some(ref map_path) = project.paths.llm.map {
        all_diags.extend(check::llm_map::check_llm_map(root, map_path));
    }

    if !project.constraints.is_empty() {
        all_diags.extend(check::constraints::check_constraints(root, &project));
        // CST-050: run rule-level check_commands
        let (cst050, _) = check::constraints::run_constraint_checks(root, &project, None, None);
        all_diags.extend(cst050);
        // CST-060: run file-level check_commands
        let (cst060, _) = check::constraints::run_file_level_checks(root, &project, None);
        all_diags.extend(cst060);
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

    let code = check::exit_code(&all_diags);
    Ok((all_diags, code))
}

fn run_checks(root: &Path) -> Result<i32> {
    style::header("check");

    let mut all_diags: Vec<Diagnostic> = Vec::new();

    // 1. Project map
    style::section("Project map");
    let project_diags = check::project_map::check_project_map(root);
    print_diags(&project_diags);
    let has_prj_fatal = project_diags.iter().any(|d| d.code == "PRJ-001");
    all_diags.extend(project_diags);

    // If project.yaml failed to parse, report diagnostics and exit without panic
    if has_prj_fatal {
        style::separator();
        let errors = all_diags
            .iter()
            .filter(|d| matches!(d.severity, check::Severity::Error))
            .count();
        println!(
            "\n  {} — {} error(s), 0 warning(s), 0 info",
            "FAILED".red().bold(),
            errors
        );
        println!();
        return Ok(1);
    }

    // Load project for further checks (safe — parse already succeeded above)
    let project = ProjectMap::load(&root.join("project.yaml"))?;
    let glossary_path = root.join(&project.paths.human.glossary);
    let glossary = match Glossary::load(&glossary_path) {
        Ok(g) => g,
        Err(e) => {
            if glossary_path.exists() {
                let diag = Diagnostic::error("GLO-001", format!("Cannot parse glossary: {}", e))
                    .with_file(&project.paths.human.glossary);
                style::section("Glossary");
                diag.print();
                all_diags.push(diag);
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

    // Load milestone info
    let (milestone_info, mst_diags) = load_milestone_info(root);
    if !mst_diags.is_empty() {
        style::section("Milestones");
        print_diags(&mst_diags);
    }
    all_diags.extend(mst_diags);
    let (contracts, trace_path_str, phase_label) = match &milestone_info {
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

    if let Some((_, milestone_id)) = &milestone_info {
        style::detail("Milestone", milestone_id);
    }

    // 2. Contracts
    style::section("Contracts");
    let contract_diags = check::contracts::check_contracts(root, &contracts, &glossary);
    print_diags(&contract_diags);
    all_diags.extend(contract_diags);

    // 3. Test specs
    style::section("Test specs");
    let test_diags = check::validation::check_test_specs(root, &contracts);
    print_diags(&test_diags);
    all_diags.extend(test_diags);

    // 4. Traceability
    style::section("Traceability");
    if root.join(&trace_path_str).exists() {
        let trace_diags =
            check::traceability::check_traceability(root, &trace_path_str, &contracts);
        print_diags(&trace_diags);
        all_diags.extend(trace_diags);
    } else {
        let diag = check::Diagnostic::info(
            "TRC-001",
            format!("Traceability file not found: {}", trace_path_str),
        )
        .with_file(&trace_path_str);
        diag.print();
        all_diags.push(diag);
    }

    // 5. Plan
    style::section("Plan");
    if let Some((_, milestone_id)) = &milestone_info {
        let plan_diags = check::plan::check_stage_plans(root, milestone_id, &contracts);
        print_diags(&plan_diags);
        all_diags.extend(plan_diags);
    } else {
        style::ok("no plan to validate");
    }

    // 6. Stack
    if let Some(ref stack) = project.stack {
        style::section("Stack");
        let stack_diags = check::stack::check_stack(stack);
        print_diags(&stack_diags);
        all_diags.extend(stack_diags);
    }

    // 7. Code traceability (@hlv markers)
    {
        style::section("Code traceability");
        let tests_path = project.paths.llm.tests.as_deref();
        let code_diags = check::code_trace::check_code_trace(
            root,
            &contracts,
            &project.constraints,
            &project.paths.llm.src,
            tests_path,
        );
        print_diags(&code_diags);
        all_diags.extend(code_diags);
    }

    // 8. LLM map (llm/map.yaml)
    if let Some(ref map_path) = project.paths.llm.map {
        style::section("LLM map");
        let map_diags = check::llm_map::check_llm_map(root, map_path);
        print_diags(&map_diags);
        all_diags.extend(map_diags);
    }

    // 9. Constraints validation
    if !project.constraints.is_empty() {
        style::section("Constraints");
        let cst_diags = check::constraints::check_constraints(root, &project);
        print_diags(&cst_diags);
        all_diags.extend(cst_diags);

        // 9b. Constraint check commands (CST-050/060)
        let (cst050, _) = check::constraints::run_constraint_checks(root, &project, None, None);
        if !cst050.is_empty() {
            print_diags(&cst050);
        }
        all_diags.extend(cst050);

        let (cst060, _) = check::constraints::run_file_level_checks(root, &project, None);
        if !cst060.is_empty() {
            print_diags(&cst060);
        }
        all_diags.extend(cst060);
    }

    // 10. Gates policy (parse validation)
    {
        let gates_path = root.join(&project.paths.validation.gates_policy);
        if gates_path.exists() {
            if let Err(e) = GatesPolicy::load(&gates_path) {
                style::section("Gates policy");
                let diag =
                    Diagnostic::error("GAT-001", format!("Cannot parse gates-policy.yaml: {}", e))
                        .with_file(&project.paths.validation.gates_policy);
                diag.print();
                all_diags.push(diag);
            }
        }
    }

    // 11. Task diagnostics (TSK-010..050)
    {
        let task_diags = check::tasks::check_tasks(root);
        if !task_diags.is_empty() {
            style::section("Tasks");
            print_diags(&task_diags);
        }
        all_diags.extend(task_diags);
    }

    // Phase-aware downgrade
    let downgraded = if let Some((milestones, _)) = &milestone_info {
        // Use stage status for milestone mode
        if let Some(stage_status) = milestones
            .current
            .as_ref()
            .and_then(|c| {
                c.stage
                    .and_then(|sid| c.stages.iter().find(|s| s.id == sid))
            })
            .map(|s| &s.status)
        {
            check::apply_phase_expectations_stage(&mut all_diags, stage_status)
        } else {
            check::apply_phase_expectations(&mut all_diags, &project.status)
        }
    } else {
        check::apply_phase_expectations(&mut all_diags, &project.status)
    };

    // Summary
    style::separator();

    println!("  phase: {}", phase_label.bold());

    let errors = all_diags
        .iter()
        .filter(|d| matches!(d.severity, check::Severity::Error))
        .count();
    let warnings = all_diags
        .iter()
        .filter(|d| matches!(d.severity, check::Severity::Warning))
        .count();
    let infos = all_diags
        .iter()
        .filter(|d| matches!(d.severity, check::Severity::Info))
        .count();

    let code = check::exit_code(&all_diags);
    let status = if code == 0 && warnings > 0 {
        "PASSED".yellow().bold()
    } else if code == 0 {
        "PASSED".green().bold()
    } else {
        "FAILED".red().bold()
    };

    println!(
        "\n  {} — {} error(s), {} warning(s), {} info",
        status, errors, warnings, infos
    );
    if downgraded > 0 {
        style::hint(&format!(
            "{} warning(s) downgraded to info (expected at {} phase)",
            downgraded, project.status
        ));
    }
    println!();

    // Run gate commands if any are configured
    if code == 0 {
        let (_, gate_failures, _) = super::gates::run_gate_commands(root, None)?;
        if gate_failures > 0 {
            return Ok(1);
        }
    }

    Ok(code)
}

fn print_diags(diags: &[Diagnostic]) {
    if diags.is_empty() {
        style::ok("all checks passed");
    } else {
        for d in diags {
            d.print();
        }
    }
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

            entries.push(ContractEntry {
                id: contract_id,
                version: "1.0.0".to_string(),
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

fn watch_loop(root: &Path) -> Result<()> {
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
        if let Err(e) = run_checks(root) {
            style::fatal(&style::format_error(&e));
        }
    }
}
