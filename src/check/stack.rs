use std::collections::HashSet;

use crate::check::Diagnostic;
use crate::model::project::Stack;

/// Validate the tech stack section of project.yaml.
pub fn check_stack(stack: &Stack) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    if stack.components.is_empty() {
        diags.push(Diagnostic::warning("STK-001", "Stack has no components"));
        return diags;
    }

    let mut seen_ids = HashSet::new();

    for comp in &stack.components {
        // STK-010: component missing id
        if comp.id.is_empty() {
            diags.push(Diagnostic::error("STK-010", "Stack component missing id"));
            continue;
        }

        // STK-011: duplicate component id
        if !seen_ids.insert(&comp.id) {
            diags.push(Diagnostic::error(
                "STK-011",
                format!("Duplicate stack component id: {}", comp.id),
            ));
        }

        // STK-012: component missing languages (only for types that should have them)
        if comp.languages.is_empty() && comp.component_type.expects_language() {
            diags.push(Diagnostic::warning(
                "STK-012",
                format!("Stack component '{}' has no languages", comp.id),
            ));
        }

        // Check dependencies
        let mut seen_deps = HashSet::new();
        for dep in &comp.dependencies {
            // STK-020: dependency missing name
            if dep.name.is_empty() {
                diags.push(Diagnostic::error(
                    "STK-020",
                    format!("Dependency missing name in component '{}'", comp.id),
                ));
                continue;
            }

            // STK-021: duplicate dep name within component
            if !seen_deps.insert(&dep.name) {
                diags.push(Diagnostic::warning(
                    "STK-021",
                    format!(
                        "Duplicate dependency '{}' in component '{}'",
                        dep.name, comp.id
                    ),
                ));
            }
        }
    }

    diags
}
