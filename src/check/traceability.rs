use std::collections::HashSet;
use std::path::Path;

use crate::check::Diagnostic;
use crate::model::project::ContractEntry;
use crate::model::traceability::TraceabilityMap;
use crate::parse::markdown;

/// Validate traceability map.
pub fn check_traceability(
    root: &Path,
    trace_path: &str,
    entries: &[ContractEntry],
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let full_path = root.join(trace_path);

    let trace = match TraceabilityMap::load(&full_path) {
        Ok(t) => t,
        Err(e) => {
            diags.push(
                Diagnostic::error("TRC-001", format!("Cannot parse traceability: {}", e))
                    .with_file(trace_path),
            );
            return diags;
        }
    };

    let contract_ids: HashSet<&str> = entries.iter().map(|e| e.id.as_str()).collect();

    // Collect known test IDs from test specs
    let known_test_ids = collect_test_ids(root, entries);

    // Collect known gate IDs from gates policy
    let known_gate_ids = collect_gate_ids(root, entries);

    // Check each mapping references existing contracts
    for mapping in &trace.mappings {
        for cid in &mapping.contracts {
            if !contract_ids.contains(cid.as_str()) {
                diags.push(
                    Diagnostic::error(
                        "TRC-010",
                        format!(
                            "Mapping for {} references unknown contract: {}",
                            mapping.requirement, cid
                        ),
                    )
                    .with_file(trace_path),
                );
            }
        }

        // Check requirement ID exists in requirements list
        let req_ids: HashSet<&str> = trace.requirements.iter().map(|r| r.id.as_str()).collect();
        if !req_ids.contains(mapping.requirement.as_str()) {
            diags.push(
                Diagnostic::error(
                    "TRC-011",
                    format!(
                        "Mapping references unknown requirement: {}",
                        mapping.requirement
                    ),
                )
                .with_file(trace_path),
            );
        }

        // Each mapping should have at least one test (unless it has no contracts — infra-only)
        if mapping.tests.is_empty() {
            if mapping.contracts.is_empty() {
                diags.push(
                    Diagnostic::info(
                        "TRC-020",
                        format!(
                            "Requirement {} has no tests mapped (infra-only, no contracts)",
                            mapping.requirement
                        ),
                    )
                    .with_file(trace_path),
                );
            } else {
                diags.push(
                    Diagnostic::warning(
                        "TRC-020",
                        format!("Requirement {} has no tests mapped", mapping.requirement),
                    )
                    .with_file(trace_path),
                );
            }
        }

        // Validate test references exist in test specs
        for test_id in &mapping.tests {
            if !known_test_ids.contains(test_id.as_str()) {
                diags.push(
                    Diagnostic::warning(
                        "TRC-022",
                        format!(
                            "Mapping for {} references unknown test: {}",
                            mapping.requirement, test_id
                        ),
                    )
                    .with_file(trace_path),
                );
            }
        }

        // Each mapping should have at least one gate
        if mapping.runtime_gates.is_empty() {
            diags.push(
                Diagnostic::warning(
                    "TRC-021",
                    format!("Requirement {} has no gates mapped", mapping.requirement),
                )
                .with_file(trace_path),
            );
        }

        // Validate gate references exist in gates policy
        for gate_ref in &mapping.runtime_gates {
            if !known_gate_ids.contains(gate_ref.as_str()) {
                diags.push(
                    Diagnostic::warning(
                        "TRC-023",
                        format!(
                            "Mapping for {} references unknown gate: {}",
                            mapping.requirement, gate_ref
                        ),
                    )
                    .with_file(trace_path),
                );
            }
        }
    }

    // Check all requirements have at least one mapping
    let mapped_reqs: HashSet<&str> = trace
        .mappings
        .iter()
        .map(|m| m.requirement.as_str())
        .collect();
    for req in &trace.requirements {
        if !mapped_reqs.contains(req.id.as_str()) {
            diags.push(
                Diagnostic::warning(
                    "TRC-030",
                    format!("Requirement {} has no traceability mapping", req.id),
                )
                .with_file(trace_path),
            );
        }
    }

    diags
}

/// Extract test IDs from all test spec files (### ID: Title pattern).
fn collect_test_ids(root: &Path, entries: &[ContractEntry]) -> HashSet<String> {
    let mut ids = HashSet::new();
    for entry in entries {
        let spec_path = match &entry.test_spec {
            Some(p) => p,
            None => continue,
        };
        let full_path = root.join(spec_path);
        let text = match std::fs::read_to_string(&full_path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let sections = markdown::extract_sections(&text);
        for section in &sections {
            for line in section.body.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("### ") {
                    if let Some(colon) = rest.find(':') {
                        ids.insert(rest[..colon].trim().to_string());
                    }
                }
            }
        }
    }
    ids
}

/// Extract gate IDs from the gates policy file.
fn collect_gate_ids(root: &Path, entries: &[ContractEntry]) -> HashSet<String> {
    let mut ids = HashSet::new();
    // Try to load gates policy from the project map
    let project_path = root.join("project.yaml");
    if let Ok(project) = crate::model::project::ProjectMap::load(&project_path) {
        let policy_path = root.join(&project.paths.validation.gates_policy);
        if let Ok(policy) = crate::model::policy::GatesPolicy::load(&policy_path) {
            for gate in &policy.gates {
                ids.insert(gate.id.clone());
            }
        }
    }
    // Also accept bare contract entry IDs as fallback — not needed but avoids
    // false positives if the user used contract IDs as gate refs
    let _ = entries;
    ids
}
