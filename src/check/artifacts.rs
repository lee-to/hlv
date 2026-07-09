use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::check::Diagnostic;
use crate::model::artifact::ArtifactGraph;
use crate::model::milestone::MilestoneMap;
use crate::model::project::ProjectMap;

pub fn check_artifacts(root: &Path, project: &ProjectMap) -> Vec<Diagnostic> {
    let milestone_id = current_milestone_id(root);
    let graph = match ArtifactGraph::load(root, project, milestone_id.as_deref()) {
        Ok(graph) => graph,
        Err(e) => {
            return vec![Diagnostic::error(
                "ART-001",
                format!("Cannot parse artifact graph: {}", e),
            )];
        }
    };

    let mut diags = Vec::new();
    let ids: BTreeSet<&str> = graph.nodes.keys().map(String::as_str).collect();

    for node in graph.nodes.values() {
        let file = node
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "project.yaml".to_string());

        if node.owners.is_empty() {
            diags.push(
                Diagnostic::warning("ART-010", format!("Artifact '{}' has no owners", node.id))
                    .with_file(&file),
            );
        }

        for relation in &node.relations {
            if !ids.contains(relation.target.as_str()) {
                diags.push(
                    Diagnostic::error(
                        "ART-020",
                        format!(
                            "Artifact '{}' has dangling {} reference '{}'",
                            node.id, relation.kind, relation.target
                        ),
                    )
                    .with_file(&file),
                );
            }
        }

        if node.artifact_type == "adr"
            && node.status.as_deref() == Some("accepted")
            && !node.relations.iter().any(|r| {
                r.kind == "affects"
                    && graph
                        .nodes
                        .get(&r.target)
                        .map(|target| target.artifact_type == "architecture")
                        .unwrap_or(false)
            })
        {
            diags.push(
                Diagnostic::warning(
                    "ART-030",
                    format!(
                        "Accepted ADR '{}' should affect an architecture artifact or be reviewed explicitly",
                        node.id
                    ),
                )
                .with_file(&file),
            );
        }

        for conflict in node.relations.iter().filter(|r| r.kind == "conflicts_with") {
            if let Some(other) = graph.nodes.get(&conflict.target) {
                if node.status.as_deref() == Some("accepted")
                    && other.status.as_deref() == Some("accepted")
                {
                    diags.push(
                        Diagnostic::error(
                            "ART-040",
                            format!(
                                "Accepted artifact '{}' conflicts with accepted artifact '{}'",
                                node.id, other.id
                            ),
                        )
                        .with_file(&file),
                    );
                }
            }
        }
    }

    diags.extend(check_artifact_markers(root, project));
    diags.extend(check_artifact_path_isolation(project));

    diags
}

fn check_artifact_path_isolation(project: &ProjectMap) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let Some(config) = &project.artifact_graph else {
        return diags;
    };

    let llm_src = project.paths.llm.src.as_deref().map(normalize_path_root);
    let llm_tests = project.paths.llm.tests.as_deref().map(normalize_path_root);

    for (node_id, entry) in &config.code_ownership {
        let source_owned = node_id.starts_with("code-") || !entry.implements.is_empty();
        let test_owned = node_id.starts_with("tests-") || !entry.verifies.is_empty();

        for path in &entry.paths {
            let normalized = normalize_ownership_path(path);
            if source_owned {
                match &llm_src {
                    Some(prefix) if is_under_path(&normalized, prefix) => {}
                    Some(prefix) => diags.push(
                        Diagnostic::error(
                            "MAP-080",
                            format!(
                                "generated implementation path is outside paths.llm.src: {} (expected prefix: {})",
                                path, prefix
                            ),
                        )
                        .with_file("project.yaml"),
                    ),
                    None => diags.push(
                        Diagnostic::error(
                            "MAP-080",
                            format!(
                                "generated implementation path is configured but paths.llm.src is missing: {}",
                                path
                            ),
                        )
                        .with_file("project.yaml"),
                    ),
                }
            }

            if test_owned {
                match &llm_tests {
                    Some(prefix) if is_under_path(&normalized, prefix) => {}
                    Some(_) => diags.push(
                        Diagnostic::error(
                            "MAP-081",
                            format!(
                                "generated test path is outside paths.llm.tests: {} (expected prefix: {})",
                                path,
                                project.paths.llm.tests.as_deref().unwrap_or("")
                            ),
                        )
                        .with_file("project.yaml"),
                    ),
                    None => diags.push(
                        Diagnostic::error(
                            "MAP-081",
                            format!(
                                "generated test path is configured but paths.llm.tests is missing: {}",
                                path
                            ),
                        )
                        .with_file("project.yaml"),
                    ),
                }
            }
        }
    }

    diags
}

fn check_artifact_markers(root: &Path, project: &ProjectMap) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let Some(config) = &project.artifact_graph else {
        return diags;
    };

    for (node_id, entry) in &config.code_ownership {
        if entry.paths.is_empty() {
            continue;
        }

        let files = collect_owned_files(root, &entry.paths);
        if files.is_empty() {
            continue;
        }

        let markers = scan_artifact_markers(&files);
        for (relation, target) in expected_marker_relations(entry) {
            if !markers.iter().any(|marker| {
                marker.node_id == *node_id
                    && marker.relation == relation
                    && marker.artifact_id == *target
            }) {
                diags.push(Diagnostic::warning(
                    "ART-050",
                    format!(
                        "Ownership node '{}' {} '{}' but no @hlv:artifact marker found in owned paths",
                        node_id, relation, target
                    ),
                ));
            }
        }
    }

    diags
}

fn expected_marker_relations(
    entry: &crate::model::project::CodeOwnershipEntry,
) -> Vec<(&'static str, &String)> {
    let mut expected = Vec::new();
    for target in &entry.requires {
        expected.push(("requires", target));
    }
    for target in &entry.depends_on {
        expected.push(("requires", target));
    }
    for target in &entry.implements {
        expected.push(("implements", target));
    }
    for target in &entry.verifies {
        expected.push(("verifies", target));
    }
    for target in &entry.documents {
        expected.push(("documents", target));
    }
    expected
}

#[derive(Debug)]
struct ArtifactMarker {
    node_id: String,
    relation: String,
    artifact_id: String,
}

fn scan_artifact_markers(files: &[PathBuf]) -> Vec<ArtifactMarker> {
    let re = Regex::new(r"@hlv:artifact\s+(\S+)\s+(\S+)\s+(\S+)").expect("valid regex");
    let mut markers = Vec::new();
    for file in files {
        let content = match std::fs::read_to_string(file) {
            Ok(content) => content,
            Err(_) => continue,
        };
        for line in content.lines() {
            for cap in re.captures_iter(line) {
                markers.push(ArtifactMarker {
                    node_id: cap[1].to_string(),
                    relation: cap[2].to_string(),
                    artifact_id: cap[3].to_string(),
                });
            }
        }
    }
    markers
}

fn collect_owned_files(root: &Path, patterns: &[String]) -> Vec<PathBuf> {
    let mut files = BTreeSet::new();
    for pattern in patterns {
        if let Some(dir_pattern) = pattern.strip_suffix("/**") {
            let dir = root.join(dir_pattern);
            if dir.is_dir() {
                collect_files_recursive(&dir, &mut files);
                continue;
            }
        }

        let full = root.join(pattern);
        if full.is_file() {
            files.insert(full);
            continue;
        }
        if full.is_dir() {
            collect_files_recursive(&full, &mut files);
            continue;
        }

        let glob_pattern = full.to_string_lossy().to_string();
        if let Ok(paths) = glob::glob(&glob_pattern) {
            for path in paths.flatten().filter(|path| path.is_file()) {
                files.insert(path);
            }
        }
    }
    files.into_iter().collect()
}

fn collect_files_recursive(dir: &Path, files: &mut BTreeSet<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if matches!(name.as_str(), ".git" | "target" | "node_modules") {
                continue;
            }
            collect_files_recursive(&path, files);
        } else if path.is_file() {
            files.insert(path);
        }
    }
}

fn normalize_ownership_path(path: &str) -> String {
    let mut normalized = path.replace('\\', "/");
    for suffix in ["/**", "/*", "/"] {
        if let Some(stripped) = normalized.strip_suffix(suffix) {
            normalized = stripped.to_string();
            break;
        }
    }
    normalized
}

fn normalize_path_root(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_string()
}

fn is_under_path(path: &str, root: &str) -> bool {
    path == root || path.starts_with(&format!("{root}/"))
}

fn current_milestone_id(root: &Path) -> Option<String> {
    let path = root.join("milestones.yaml");
    if !path.exists() {
        return None;
    }
    MilestoneMap::load(&path)
        .ok()
        .and_then(|m| m.current.map(|c| c.id))
}
