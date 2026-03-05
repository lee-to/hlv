use std::collections::HashSet;
use std::path::Path;

use crate::check::Diagnostic;
use crate::model::project::ContractEntry;
use crate::parse::markdown;

/// Validate test specs in validation/test-specs/.
pub fn check_test_specs(root: &Path, entries: &[ContractEntry]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut all_test_ids: Vec<(String, String)> = Vec::new(); // (test_id, file)

    for entry in entries {
        let spec_path = match &entry.test_spec {
            Some(p) => p.clone(),
            None => {
                diags.push(
                    Diagnostic::warning(
                        "TST-001",
                        format!("No test_spec for contract {}", entry.id),
                    )
                    .with_file(&entry.path),
                );
                continue;
            }
        };

        let full_path = root.join(&spec_path);
        let text = match std::fs::read_to_string(&full_path) {
            Ok(t) => t,
            Err(e) => {
                diags.push(
                    Diagnostic::error("TST-002", format!("Cannot read test spec: {}", e))
                        .with_file(&spec_path),
                );
                continue;
            }
        };

        // Check derived_from references the contract
        let first_lines: String = text.lines().take(5).collect::<Vec<_>>().join("\n");
        if !first_lines.contains(&entry.id) && !first_lines.contains(&entry.path) {
            diags.push(
                Diagnostic::warning(
                    "TST-010",
                    format!("derived_from doesn't reference contract {}", entry.id),
                )
                .with_file(&spec_path),
            );
        }

        // Check contract_version matches the contract entry's version
        for line in text.lines().take(5) {
            if let Some(ver) = line.strip_prefix("contract_version:") {
                let spec_ver = ver.trim();
                if spec_ver != entry.version {
                    diags.push(
                        Diagnostic::warning(
                            "TST-011",
                            format!(
                                "Test spec contract_version ({}) differs from contract {} version ({})",
                                spec_ver, entry.id, entry.version
                            ),
                        )
                        .with_file(&spec_path),
                    );
                }
                break;
            }
        }

        // Extract test IDs (### ID-PATTERN: Title)
        let sections = markdown::extract_sections(&text);
        let mut has_pbt = false;
        let mut has_contract_test = false;

        for section in &sections {
            for line in section.body.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("### ") {
                    if let Some(colon) = rest.find(':') {
                        let test_id = rest[..colon].trim().to_string();
                        all_test_ids.push((test_id.clone(), spec_path.clone()));

                        if test_id.starts_with("CT-") {
                            has_contract_test = true;
                        } else if test_id.starts_with("PBT-") {
                            has_pbt = true;
                        }
                    }
                }
            }
        }

        // Check: at least one contract test and one PBT per invariant
        if !has_contract_test {
            diags.push(
                Diagnostic::warning("TST-020", "No contract tests (CT-*) found")
                    .with_file(&spec_path),
            );
        }

        // Check gate references exist in body
        let has_gate_ref = text.contains("GATE-");
        if !has_gate_ref {
            diags.push(
                Diagnostic::warning("TST-030", "No gate references found in test spec")
                    .with_file(&spec_path),
            );
        }

        // Check PBT for each invariant
        if !has_pbt {
            diags.push(
                Diagnostic::warning("TST-021", "No property-based tests (PBT-*) found")
                    .with_file(&spec_path),
            );
        }
    }

    // Check test ID uniqueness
    let mut seen: HashSet<String> = HashSet::new();
    for (id, file) in &all_test_ids {
        if !seen.insert(id.clone()) {
            diags.push(
                Diagnostic::error("TST-040", format!("Duplicate test ID: {}", id)).with_file(file),
            );
        }
    }

    diags
}
