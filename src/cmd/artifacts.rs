use anyhow::{Context, Result};
use colored::Colorize;
use std::path::Path;

use super::style;
use crate::check::{self, Severity};
use crate::model::artifact::{ArtifactFull, ArtifactGraph, ArtifactIndex, ArtifactNode};
use crate::model::milestone::MilestoneMap;
use crate::model::project::{ArtifactGraphConfig, CodeOwnershipEntry, ProjectMap};

/// `hlv artifacts [--global | --milestone] [--json]`
pub fn run_list(root: &Path, global_only: bool, milestone_only: bool, json: bool) -> Result<()> {
    let root = &crate::config_root(root);
    let milestone_id = current_milestone_id(root);

    let index = if global_only {
        ArtifactIndex {
            global: ArtifactIndex::load_global(root)?,
            milestone: Vec::new(),
        }
    } else if milestone_only {
        let mid = milestone_id.as_deref().context("No active milestone")?;
        ArtifactIndex {
            global: Vec::new(),
            milestone: ArtifactIndex::load_milestone(root, mid)?,
        }
    } else {
        ArtifactIndex::load(root, milestone_id.as_deref())?
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&index)?);
    } else {
        if !index.global.is_empty() {
            style::header("Global artifacts");
            for a in &index.global {
                println!("  {} {} ({})", "·".dimmed(), a.name.bold(), a.kind);
            }
        }
        if !index.milestone.is_empty() {
            style::header("Milestone artifacts");
            for a in &index.milestone {
                println!("  {} {} ({})", "·".dimmed(), a.name.bold(), a.kind);
            }
        }
        if index.global.is_empty() && index.milestone.is_empty() {
            style::hint("No artifacts found.");
        }
    }
    Ok(())
}

/// `hlv artifacts show <name> [--global | --milestone] [--json]`
pub fn run_show(
    root: &Path,
    name: &str,
    global_only: bool,
    milestone_only: bool,
    json: bool,
) -> Result<()> {
    let root = &crate::config_root(root);
    let milestone_id = current_milestone_id(root);

    // Search in appropriate scope
    let artifact = if global_only {
        find_artifact(root, name, true, None)?
    } else if milestone_only {
        let mid = milestone_id.as_deref().context("No active milestone")?;
        find_artifact(root, name, false, Some(mid))?
    } else {
        // Try milestone first, then global
        let mid = milestone_id.as_deref();
        if let Some(mid) = mid {
            find_artifact(root, name, false, Some(mid))
                .or_else(|_| find_artifact(root, name, true, None))?
        } else {
            find_artifact(root, name, true, None)?
        }
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&artifact)?);
    } else {
        println!("{}", artifact.content);
    }
    Ok(())
}

/// Returns artifact index as data (no stdout).
pub fn get_artifacts_list(
    root: &Path,
    global_only: bool,
    milestone_only: bool,
) -> Result<ArtifactIndex> {
    let root = &crate::config_root(root);
    let milestone_id = current_milestone_id(root);

    if global_only {
        Ok(ArtifactIndex {
            global: ArtifactIndex::load_global(root)?,
            milestone: Vec::new(),
        })
    } else if milestone_only {
        let mid = milestone_id.as_deref().context("No active milestone")?;
        Ok(ArtifactIndex {
            global: Vec::new(),
            milestone: ArtifactIndex::load_milestone(root, mid)?,
        })
    } else {
        ArtifactIndex::load(root, milestone_id.as_deref())
    }
}

/// Returns a single artifact as data (no stdout).
pub fn get_artifact_show(
    root: &Path,
    name: &str,
    global_only: bool,
    milestone_only: bool,
) -> Result<ArtifactFull> {
    let root = &crate::config_root(root);
    let milestone_id = current_milestone_id(root);

    if global_only {
        find_artifact(root, name, true, None)
    } else if milestone_only {
        let mid = milestone_id.as_deref().context("No active milestone")?;
        find_artifact(root, name, false, Some(mid))
    } else {
        let mid = milestone_id.as_deref();
        if let Some(mid) = mid {
            find_artifact(root, name, false, Some(mid))
                .or_else(|_| find_artifact(root, name, true, None))
        } else {
            find_artifact(root, name, true, None)
        }
    }
}

/// `hlv artifacts impact <id-or-path> [--json]`
pub fn run_impact(
    root: &Path,
    target: Option<&str>,
    changed: bool,
    base: Option<&str>,
    json: bool,
) -> Result<()> {
    let root = &crate::config_root(root);
    let project = ProjectMap::load(&root.join("project.yaml"))?;
    let milestone_id = current_milestone_id(root);
    let graph = ArtifactGraph::load(root, &project, milestone_id.as_deref())?;
    let changed_ids = if changed {
        changed_artifact_ids(root, &graph, base)?
    } else {
        vec![resolve_artifact_target(
            root,
            &graph,
            target.context("Artifact id or path required")?,
        )?]
    };

    let report = graph.impact(&changed_ids);
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    style::header("artifact impact");
    if report.changed.is_empty() {
        style::hint("No changed artifacts found.");
        return Ok(());
    }
    style::section("Changed");
    for id in &report.changed {
        println!("  {} {}", "·".dimmed(), id.bold());
    }
    style::section("Expected to review");
    if report.affected.is_empty() {
        style::ok("no downstream artifacts");
    } else {
        for item in &report.affected {
            let path = item
                .path
                .as_ref()
                .map(|p| format!(" {}", p.display().to_string().dimmed()))
                .unwrap_or_default();
            let owners = if item.owners.is_empty() {
                "owners: unknown".to_string()
            } else {
                format!("owners: {}", item.owners.join(", "))
            };
            println!(
                "  {} {} ({}){} — {}",
                "·".dimmed(),
                item.id.bold(),
                item.artifact_type,
                path,
                owners
            );
            println!("    via: {}", item.via.join(", "));
        }
    }
    Ok(())
}

/// `hlv artifacts graph [--json]`
pub fn run_graph(root: &Path, json: bool) -> Result<()> {
    let root = &crate::config_root(root);
    let project = ProjectMap::load(&root.join("project.yaml"))?;
    let milestone_id = current_milestone_id(root);
    let graph = ArtifactGraph::load(root, &project, milestone_id.as_deref())?;
    let report = ArtifactGraphReport::from_graph(&graph);

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    style::header("artifact graph");
    if report.nodes.is_empty() {
        style::hint("No artifact graph metadata found.");
        return Ok(());
    }

    style::section("Nodes");
    for node in &report.nodes {
        let location = node
            .path
            .as_ref()
            .map(|path| path.display().to_string())
            .or_else(|| {
                if node.paths.is_empty() {
                    None
                } else {
                    Some(node.paths.join(", "))
                }
            })
            .map(|location| format!(" {}", location.dimmed()))
            .unwrap_or_default();
        let owners = if node.owners.is_empty() {
            "owners: unknown".to_string()
        } else {
            format!("owners: {}", node.owners.join(", "))
        };
        println!(
            "  {} {} ({}){} — {}",
            "·".dimmed(),
            node.id.bold(),
            node.artifact_type,
            location,
            owners
        );
    }

    style::section("Relations");
    if report.edges.is_empty() {
        style::ok("no relations");
    } else {
        for edge in &report.edges {
            println!(
                "  {} {} --{}--> {}",
                "·".dimmed(),
                edge.source.bold(),
                edge.relation,
                edge.target.bold()
            );
        }
    }

    Ok(())
}

/// `hlv artifacts audit [--json]`
pub fn run_audit(root: &Path, json: bool) -> Result<()> {
    let root = &crate::config_root(root);
    let project = ProjectMap::load(&root.join("project.yaml"))?;
    let milestone_id = current_milestone_id(root);
    let graph = ArtifactGraph::load(root, &project, milestone_id.as_deref());
    let diags = check::artifacts::check_artifacts(root, &project);
    let exit_code = check::exit_code(&diags);
    let errors = diags
        .iter()
        .filter(|d| matches!(d.severity, Severity::Error))
        .count();
    let warnings = diags
        .iter()
        .filter(|d| matches!(d.severity, Severity::Warning))
        .count();
    let metadata_found = graph
        .as_ref()
        .map(|graph| !graph.nodes.is_empty())
        .unwrap_or(true);
    if json {
        let output = serde_json::json!({
            "metadata_found": metadata_found,
            "errors": errors,
            "warnings": warnings,
            "exit_code": exit_code,
            "diagnostics": diags,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        if exit_code != 0 {
            std::process::exit(exit_code);
        }
        return Ok(());
    }
    style::header("artifact audit");
    if !metadata_found {
        style::hint(
            "No artifact graph metadata found. Existing projects are compatible; add artifact frontmatter and project.yaml -> artifact_graph.code_ownership when you want impact analysis.",
        );
        return Ok(());
    }
    if diags.is_empty() {
        style::ok("artifact graph checks passed");
    } else {
        for diag in &diags {
            diag.print();
        }
    }
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}

/// `hlv artifacts sync [--check] [--json]`
pub fn run_sync(root: &Path, check: bool, json: bool) -> Result<()> {
    let root = &crate::config_root(root);
    let project_path = root.join("project.yaml");
    let mut project = ProjectMap::load(&project_path)?;
    let milestone_id = current_milestone_id(root);
    let graph = ArtifactGraph::load(root, &project, milestone_id.as_deref())?;
    let missing = missing_ownership_targets(&graph);

    if !check && !missing.is_empty() {
        let config = project
            .artifact_graph
            .get_or_insert_with(|| ArtifactGraphConfig {
                code_ownership: Default::default(),
            });
        for target in &missing {
            config
                .code_ownership
                .entry(target.id.clone())
                .or_insert_with(|| CodeOwnershipEntry {
                    paths: Vec::new(),
                    owners: target.owners.clone(),
                    requires: Vec::new(),
                    implements: Vec::new(),
                    verifies: Vec::new(),
                    documents: Vec::new(),
                    depends_on: Vec::new(),
                });
        }
        project.save(&project_path)?;
    }

    if json {
        let output = serde_json::json!({
            "changed": !check && !missing.is_empty(),
            "check": check,
            "missing": missing,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        if check && !output["missing"].as_array().unwrap().is_empty() {
            std::process::exit(1);
        }
        return Ok(());
    }

    style::header("artifact sync");
    if missing.is_empty() {
        style::ok("project.yaml artifact_graph is in sync");
        return Ok(());
    }

    if check {
        for target in &missing {
            println!(
                "  {} {} referenced by {} needs project.yaml -> artifact_graph.code_ownership entry",
                "!".yellow().bold(),
                target.id.bold(),
                target.referenced_by.join(", ")
            );
        }
        std::process::exit(1);
    }

    for target in &missing {
        println!(
            "  {} added {} (owners: {})",
            "+".green().bold(),
            target.id.bold(),
            if target.owners.is_empty() {
                "unknown".to_string()
            } else {
                target.owners.join(", ")
            }
        );
    }
    style::hint("Add concrete paths under each new code_ownership entry before relying on path-based routing.");
    Ok(())
}

fn find_artifact(
    root: &Path,
    name: &str,
    global: bool,
    milestone_id: Option<&str>,
) -> Result<ArtifactFull> {
    let dir = if global {
        root.join("human/artifacts")
    } else {
        root.join("human/milestones")
            .join(milestone_id.unwrap())
            .join("artifacts")
    };
    let path = dir.join(format!("{name}.md"));
    anyhow::ensure!(path.exists(), "Artifact '{}' not found", name);
    ArtifactFull::load(&path)
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

fn resolve_artifact_target(root: &Path, graph: &ArtifactGraph, target: &str) -> Result<String> {
    if graph.nodes.contains_key(target) {
        return Ok(target.to_string());
    }
    let target_path = root.join(target);
    let normalized = target_path.strip_prefix(root).unwrap_or(&target_path);
    let normalized = normalize_relative_path(normalized);
    for node in graph.nodes.values() {
        if node
            .path
            .as_ref()
            .map(|p| normalize_relative_path(p) == normalized)
            .unwrap_or(false)
            || node
                .paths
                .iter()
                .any(|pattern| path_matches_pattern(&normalized, pattern))
        {
            return Ok(node.id.clone());
        }
    }
    anyhow::bail!("Unknown artifact id or path '{}'", target);
}

fn changed_artifact_ids(
    root: &Path,
    graph: &ArtifactGraph,
    base: Option<&str>,
) -> Result<Vec<String>> {
    let changed_paths = changed_paths(root, base)?;
    Ok(graph
        .nodes
        .values()
        .filter(|node| node_matches_changed_paths(node, &changed_paths))
        .map(|node| node.id.clone())
        .collect())
}

fn changed_paths(root: &Path, base: Option<&str>) -> Result<std::collections::BTreeSet<String>> {
    if let Some(base) = base {
        let merge_base = std::process::Command::new("git")
            .args(["merge-base", base, "HEAD"])
            .current_dir(root)
            .output()
            .with_context(|| format!("Cannot run git merge-base {base} HEAD"))?;
        if !merge_base.status.success() {
            let stderr = String::from_utf8_lossy(&merge_base.stderr);
            anyhow::bail!(
                "git merge-base failed: {}. If this is a shallow CI checkout, fetch the base ref first or use actions/checkout with fetch-depth: 0.",
                stderr.trim()
            );
        }
        let merge_base = String::from_utf8_lossy(&merge_base.stdout)
            .trim()
            .to_string();
        let output = std::process::Command::new("git")
            .args(["diff", "--name-only", &merge_base, "HEAD"])
            .current_dir(root)
            .output()
            .with_context(|| format!("Cannot run git diff --name-only {merge_base} HEAD"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git diff failed: {}", stderr.trim());
        }
        return Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(str::to_string)
            .collect());
    }

    let output = std::process::Command::new("git")
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .current_dir(root)
        .output()
        .context("Cannot run git status --porcelain")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git status failed: {}", stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_git_status_path)
        .map(str::to_string)
        .collect())
}

fn node_matches_changed_paths(
    node: &crate::model::artifact::ArtifactNode,
    changed_paths: &std::collections::BTreeSet<String>,
) -> bool {
    if node
        .path
        .as_ref()
        .map(|p| changed_paths.contains(&normalize_relative_path(p)))
        .unwrap_or(false)
    {
        return true;
    }
    changed_paths.iter().any(|changed_path| {
        node.paths
            .iter()
            .any(|pattern| path_matches_pattern(changed_path, pattern))
    })
}

fn normalize_relative_path(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches("./")
        .replace('\\', "/")
}

fn path_matches_pattern(path: &str, pattern: &str) -> bool {
    let pattern = pattern.trim_start_matches("./");
    if path == pattern {
        return true;
    }
    if let Some(dir) = pattern.strip_suffix("/**") {
        return path == dir || path.starts_with(&format!("{dir}/"));
    }
    glob::Pattern::new(pattern)
        .map(|pattern| pattern.matches(path))
        .unwrap_or(false)
}

fn parse_git_status_path(line: &str) -> Option<&str> {
    if line.len() < 4 {
        return None;
    }
    let path = line[3..].trim();
    if path.is_empty() {
        return None;
    }
    Some(path.rsplit_once(" -> ").map(|(_, new)| new).unwrap_or(path))
}

#[derive(Debug, serde::Serialize)]
struct ArtifactGraphReport {
    nodes: Vec<ArtifactNode>,
    edges: Vec<ArtifactGraphEdge>,
}

impl ArtifactGraphReport {
    fn from_graph(graph: &ArtifactGraph) -> Self {
        let nodes: Vec<ArtifactNode> = graph.nodes.values().cloned().collect();
        let edges = graph
            .nodes
            .values()
            .flat_map(|node| {
                node.relations
                    .iter()
                    .map(move |relation| ArtifactGraphEdge {
                        source: node.id.clone(),
                        relation: relation.kind.clone(),
                        target: relation.target.clone(),
                    })
            })
            .collect();

        Self { nodes, edges }
    }
}

#[derive(Debug, serde::Serialize)]
struct ArtifactGraphEdge {
    source: String,
    relation: String,
    target: String,
}

#[derive(Debug, serde::Serialize)]
struct SyncTarget {
    id: String,
    owners: Vec<String>,
    referenced_by: Vec<String>,
}

fn missing_ownership_targets(graph: &ArtifactGraph) -> Vec<SyncTarget> {
    let mut targets: std::collections::BTreeMap<String, SyncTarget> =
        std::collections::BTreeMap::new();
    for node in graph.nodes.values().filter(|node| node.path.is_some()) {
        for relation in &node.relations {
            if graph.nodes.contains_key(&relation.target) || !is_ownership_target(&relation.target)
            {
                continue;
            }
            let target = targets
                .entry(relation.target.clone())
                .or_insert_with(|| SyncTarget {
                    id: relation.target.clone(),
                    owners: node.owners.clone(),
                    referenced_by: Vec::new(),
                });
            if target.owners.is_empty() && !node.owners.is_empty() {
                target.owners = node.owners.clone();
            }
            if !target.referenced_by.iter().any(|id| id == &node.id) {
                target.referenced_by.push(node.id.clone());
            }
        }
    }
    targets.into_values().collect()
}

fn is_ownership_target(id: &str) -> bool {
    id.starts_with("code-")
        || id.starts_with("tests-")
        || id.starts_with("docs-")
        || id.starts_with("clients-")
}

// Display for ArtifactKind
impl std::fmt::Display for crate::model::artifact::ArtifactKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Context => write!(f, "context"),
            Self::Stack => write!(f, "stack"),
            Self::Constraints => write!(f, "constraints"),
            Self::Decision => write!(f, "decision"),
            Self::Feature => write!(f, "feature"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}
