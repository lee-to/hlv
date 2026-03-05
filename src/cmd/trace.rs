use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;

use super::style;
use crate::model::traceability::TraceabilityMap;

pub fn run(project_root: &Path, visual: bool, json: bool) -> Result<()> {
    if json {
        let milestones =
            crate::model::milestone::MilestoneMap::load(&project_root.join("milestones.yaml"))?;
        let current = milestones.current.as_ref().context("No active milestone")?;
        let trace_path = project_root
            .join("human/milestones")
            .join(&current.id)
            .join("traceability.yaml");
        if trace_path.exists() {
            let trace = TraceabilityMap::load(&trace_path)?;
            println!("{}", serde_json::to_string_pretty(&trace)?);
        } else {
            println!("null");
        }
        return Ok(());
    }

    style::header("trace");

    let milestones =
        crate::model::milestone::MilestoneMap::load(&project_root.join("milestones.yaml"))?;
    let current = match &milestones.current {
        Some(c) => c,
        None => {
            style::hint("No active milestone. Run `hlv milestone new <name>` to start.");
            return Ok(());
        }
    };

    let trace_path = project_root
        .join("human/milestones")
        .join(&current.id)
        .join("traceability.yaml");
    if !trace_path.exists() {
        style::hint(&format!(
            "No traceability map found for milestone '{}'.\nRun /generate to create one.",
            current.id
        ));
        return Ok(());
    }

    style::detail("Milestone", &current.id);
    let trace = TraceabilityMap::load(&trace_path)?;
    if visual {
        print_visual_trace(&trace);
    } else {
        print_table_trace(&trace);
    }

    Ok(())
}

fn print_table_trace(trace: &TraceabilityMap) {
    println!(
        "  Requirements: {} | Mappings: {}\n",
        trace.requirements.len(),
        trace.mappings.len()
    );

    for mapping in &trace.mappings {
        println!("  {} {}", "REQ".cyan().bold(), mapping.requirement.bold());
        println!("    contracts: {}", mapping.contracts.join(", "));
        println!("    tests:     {}", mapping.tests.join(", "));
        println!("    gates:     {}", mapping.runtime_gates.join(", "));
        if !mapping.scenarios.is_empty() {
            println!("    scenarios: {}", mapping.scenarios.join(", "));
        }
        println!();
    }

    // Coverage
    if let Some(ref policy) = trace.coverage_policy {
        println!("  Coverage policy:");
        println!(
            "    full_traceability: {}",
            policy.require_full_traceability
        );
        println!("    allow_unmapped: {}", policy.allow_unmapped_requirements);
        if let Some(min) = policy.minimum_mandatory_gate_coverage_percent {
            println!("    min_gate_coverage: {}%", min);
        }
    }
}

fn print_visual_trace(trace: &TraceabilityMap) {
    println!();
    println!(
        "  {:<20} {:<20} {:<30} {}",
        "REQUIREMENT".cyan().bold(),
        "CONTRACT".green().bold(),
        "TEST".yellow().bold(),
        "GATE".magenta().bold()
    );
    println!("  {}", "─".repeat(90));

    for mapping in &trace.mappings {
        let req = &mapping.requirement;
        let contracts_str = mapping.contracts.join(", ");
        let tests = &mapping.tests;
        let gates_str = mapping.runtime_gates.join(", ");

        // First test line
        if let Some(first_test) = tests.first() {
            println!(
                "  {:<20} {:<20} {:<30} {}",
                req.cyan(),
                contracts_str.green(),
                first_test.yellow(),
                gates_str.magenta()
            );
        }
        // Remaining tests
        for test in tests.iter().skip(1) {
            println!("  {:<20} {:<20} {:<30}", "", "", test.yellow());
        }

        // ASCII arrow chain
        println!(
            "  {} ──→ {} ──→ {} ──→ {}",
            req.dimmed(),
            "CTR".dimmed(),
            "TST".dimmed(),
            "GATE".dimmed()
        );
        println!();
    }
}
