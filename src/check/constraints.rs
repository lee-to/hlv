use std::path::Path;

use crate::check::Diagnostic;
use crate::model::policy::ConstraintFile;
use crate::model::project::ProjectMap;

/// CST-010: constraint files exist and parse
/// CST-020: no duplicate rule IDs within a constraint
/// CST-030: severity values are valid
pub fn check_constraints(root: &Path, project: &ProjectMap) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let valid_severities = ["critical", "high", "medium", "low"];

    for entry in &project.constraints {
        let file_path = root.join(&entry.path);

        // CST-010: file exists and parses
        if !file_path.exists() {
            diags.push(
                Diagnostic::error(
                    "CST-010",
                    format!("Constraint file not found: {}", entry.path),
                )
                .with_file(&entry.path),
            );
            continue;
        }

        let cf = match ConstraintFile::load(&file_path) {
            Ok(cf) => cf,
            Err(_) => {
                // Try as performance constraint (metric-based) — not an error
                if crate::model::policy::PerformanceConstraints::load(&file_path).is_ok() {
                    continue;
                }
                diags.push(
                    Diagnostic::error(
                        "CST-010",
                        format!("Cannot parse constraint file: {}", entry.path),
                    )
                    .with_file(&entry.path),
                );
                continue;
            }
        };

        // CST-020: no duplicate rule IDs
        let mut seen_ids = std::collections::HashSet::new();
        for rule in &cf.rules {
            if !seen_ids.insert(&rule.id) {
                diags.push(
                    Diagnostic::error(
                        "CST-020",
                        format!(
                            "Duplicate rule ID '{}' in constraint '{}'",
                            rule.id, entry.id
                        ),
                    )
                    .with_file(&entry.path),
                );
            }

            // CST-030: valid severity
            if !valid_severities.contains(&rule.severity.as_str()) {
                diags.push(
                    Diagnostic::error(
                        "CST-030",
                        format!(
                            "Invalid severity '{}' for rule '{}' in '{}'",
                            rule.severity, rule.id, entry.id
                        ),
                    )
                    .with_file(&entry.path),
                );
            }
        }
    }

    diags
}
