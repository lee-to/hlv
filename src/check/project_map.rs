use std::path::Path;

use crate::check::Diagnostic;
use crate::model::glossary::Glossary;
use crate::model::project::ProjectMap;

/// Validate the project map (project.yaml).
pub fn check_project_map(root: &Path) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let project_path = root.join("project.yaml");

    let project = match ProjectMap::load(&project_path) {
        Ok(p) => p,
        Err(e) => {
            diags.push(Diagnostic::error(
                "PRJ-001",
                format!("Cannot parse project.yaml: {}", e),
            ));
            return diags;
        }
    };

    // Check global paths
    check_path_exists(root, &project.paths.human.glossary, "PRJ-010", &mut diags);
    check_dir_exists(
        root,
        &project.paths.human.constraints,
        "PRJ-012",
        &mut diags,
    );
    check_path_exists(
        root,
        &project.paths.validation.gates_policy,
        "PRJ-014",
        &mut diags,
    );

    // Check glossary_types match actual glossary
    let glossary_path = root.join(&project.paths.human.glossary);
    if let Ok(glossary) = Glossary::load(&glossary_path) {
        let known_types = glossary.all_type_names();
        for gt in &project.glossary_types {
            if !known_types.contains(&gt.as_str()) {
                diags.push(
                    Diagnostic::warning(
                        "PRJ-030",
                        format!("glossary_type '{}' not found in glossary", gt),
                    )
                    .with_file("project.yaml"),
                );
            }
        }
    }

    // Check constraint paths
    for constraint in &project.constraints {
        check_path_exists(root, &constraint.path, "PRJ-040", &mut diags);
    }

    // Check paths.llm.src must be under llm/ — prevents agents from creating code in project root
    if !project.paths.llm.src.starts_with("llm/") {
        diags.push(
            Diagnostic::error(
                "PRJ-080",
                format!(
                    "paths.llm.src is '{}' but must be under llm/ (e.g. llm/src/). Generated code must not pollute the project root.",
                    project.paths.llm.src
                ),
            )
            .with_file("project.yaml"),
        );
    }
    if let Some(ref tests) = project.paths.llm.tests {
        if !tests.starts_with("llm/") {
            diags.push(
                Diagnostic::error(
                    "PRJ-081",
                    format!(
                        "paths.llm.tests is '{}' but must be under llm/ (e.g. llm/tests/)",
                        tests
                    ),
                )
                .with_file("project.yaml"),
            );
        }
    }

    // Check stack
    if let Some(ref stack) = project.stack {
        diags.extend(crate::check::stack::check_stack(stack));
    }

    diags
}

fn check_path_exists(root: &Path, rel: &str, code: &str, diags: &mut Vec<Diagnostic>) {
    if !root.join(rel).exists() {
        diags.push(
            Diagnostic::error(code, format!("Path not found: {}", rel)).with_file("project.yaml"),
        );
    }
}

fn check_dir_exists(root: &Path, rel: &str, code: &str, diags: &mut Vec<Diagnostic>) {
    let p = root.join(rel);
    if !p.exists() || !p.is_dir() {
        diags.push(
            Diagnostic::error(code, format!("Directory not found: {}", rel))
                .with_file("project.yaml"),
        );
    }
}
