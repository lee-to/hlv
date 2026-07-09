use std::ffi::OsStr;
use std::path::{Path, PathBuf};

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

    if project.features.legacy_mode {
        check_legacy_code_paths(root, &project, &mut diags);
    } else if project.paths.code.is_some() {
        diags.push(
            Diagnostic::warning(
                "PRJ-093",
                "paths.code is configured but features.legacy_mode is false; paths.code is only used for adopted projects.",
            )
            .with_file("project.yaml"),
        );
    }

    // paths.llm.* is the generated/agent-owned namespace and must stay under
    // llm/ in every mode; only paths.code.* may point at brownfield roots.
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

fn check_legacy_code_paths(root: &Path, project: &ProjectMap, diags: &mut Vec<Diagnostic>) {
    let Some(code_paths) = project.paths.code.as_ref() else {
        diags.push(
            Diagnostic::error(
                "PRJ-090",
                "features.legacy_mode is true but paths.code.src is not configured.",
            )
            .with_file("project.yaml"),
        );
        return;
    };

    if code_paths.src.is_empty() {
        diags.push(
            Diagnostic::error(
                "PRJ-090",
                "features.legacy_mode is true but paths.code.src is empty.",
            )
            .with_file("project.yaml"),
        );
    }

    let repo_root = repo_root_for_code_paths(root, project);
    for src in &code_paths.src {
        check_code_dir_exists(&repo_root, src, "PRJ-091", "paths.code.src", diags);
    }

    if let Some(tests) = &code_paths.tests {
        for test_root in tests {
            check_code_dir_exists(&repo_root, test_root, "PRJ-092", "paths.code.tests", diags);
        }
    }
}

fn repo_root_for_code_paths(config_root: &Path, project: &ProjectMap) -> PathBuf {
    if project.features.legacy_mode
        && config_root
            .file_name()
            .is_some_and(|name| name == OsStr::new(".hlv"))
    {
        config_root
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| config_root.to_path_buf())
    } else {
        config_root.to_path_buf()
    }
}

fn check_code_dir_exists(
    repo_root: &Path,
    rel: &str,
    code: &str,
    field: &str,
    diags: &mut Vec<Diagnostic>,
) {
    let p = repo_root.join(rel);
    if !p.exists() || !p.is_dir() {
        diags.push(
            Diagnostic::error(code, format!("{field} directory not found: {rel}"))
                .with_file("project.yaml"),
        );
    }
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
