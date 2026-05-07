use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::model::project::ProjectMap;

/// Kind of artifact based on filename
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Context,
    Stack,
    Constraints,
    Decision,
    Feature,
    Unknown,
}

/// Lightweight artifact metadata (no content loaded)
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactMeta {
    pub name: String,
    pub path: PathBuf,
    pub kind: ArtifactKind,
}

/// Full artifact with content
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactFull {
    pub name: String,
    pub path: PathBuf,
    pub kind: ArtifactKind,
    pub content: String,
}

/// Index of all artifacts (global + milestone)
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactIndex {
    pub global: Vec<ArtifactMeta>,
    pub milestone: Vec<ArtifactMeta>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ArtifactFrontmatter {
    pub id: String,
    #[serde(rename = "type")]
    pub artifact_type: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub owners: Vec<String>,
    #[serde(default)]
    pub owns: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub affects: Vec<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub implements: Vec<String>,
    #[serde(default)]
    pub verifies: Vec<String>,
    #[serde(default)]
    pub documents: Vec<String>,
    #[serde(default)]
    pub supersedes: Vec<String>,
    #[serde(default)]
    pub conflicts_with: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactNode {
    pub id: String,
    pub artifact_type: String,
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub paths: Vec<String>,
    pub owners: Vec<String>,
    pub status: Option<String>,
    pub owns: Option<String>,
    pub relations: Vec<ArtifactRelation>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ArtifactRelation {
    pub kind: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactItem {
    pub id: String,
    pub artifact_type: String,
    pub path: Option<PathBuf>,
    pub paths: Vec<String>,
    pub owners: Vec<String>,
    pub via: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactReport {
    pub changed: Vec<String>,
    pub affected: Vec<ImpactItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactGraph {
    pub nodes: BTreeMap<String, ArtifactNode>,
}

impl ArtifactIndex {
    /// Load metadata for all artifacts
    pub fn load(root: &Path, milestone_id: Option<&str>) -> Result<Self> {
        let global = Self::load_global(root)?;
        let milestone = match milestone_id {
            Some(mid) => Self::load_milestone(root, mid)?,
            None => Vec::new(),
        };
        Ok(Self { global, milestone })
    }

    /// Load global artifacts from human/artifacts/
    pub fn load_global(root: &Path) -> Result<Vec<ArtifactMeta>> {
        let dir = root.join("human/artifacts");
        scan_artifacts_dir(&dir)
    }

    /// Load milestone artifacts from human/milestones/{mid}/artifacts/
    pub fn load_milestone(root: &Path, mid: &str) -> Result<Vec<ArtifactMeta>> {
        let dir = root.join("human/milestones").join(mid).join("artifacts");
        scan_artifacts_dir(&dir)
    }
}

impl ArtifactFull {
    /// Load a single artifact with content
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let kind = classify_artifact(&name);
        Ok(Self {
            name,
            path: path.to_path_buf(),
            kind,
            content,
        })
    }
}

impl ArtifactGraph {
    pub fn load(root: &Path, project: &ProjectMap, milestone_id: Option<&str>) -> Result<Self> {
        let mut nodes = BTreeMap::new();
        let mut sources = BTreeMap::new();
        for path in artifact_markdown_paths(root, project, milestone_id)? {
            let content = std::fs::read_to_string(&path)?;
            if let Some(frontmatter) = parse_frontmatter(&content)? {
                let id = frontmatter.id.clone();
                let source = path
                    .strip_prefix(root)
                    .ok()
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                let relations = relations_from_frontmatter(&frontmatter);
                insert_node(
                    &mut nodes,
                    &mut sources,
                    &id,
                    &source,
                    ArtifactNode {
                        id: frontmatter.id,
                        artifact_type: frontmatter.artifact_type,
                        path: path.strip_prefix(root).ok().map(Path::to_path_buf),
                        paths: Vec::new(),
                        owners: frontmatter.owners,
                        status: frontmatter.status,
                        owns: frontmatter.owns,
                        relations,
                    },
                )?;
            }
        }

        if let Some(config) = &project.artifact_graph {
            for (id, entry) in &config.code_ownership {
                let mut relations = Vec::new();
                push_relations(&mut relations, "requires", &entry.requires);
                push_relations(&mut relations, "requires", &entry.depends_on);
                push_relations(&mut relations, "implements", &entry.implements);
                push_relations(&mut relations, "verifies", &entry.verifies);
                push_relations(&mut relations, "documents", &entry.documents);
                let source = format!("project.yaml -> artifact_graph.code_ownership.{id}");
                insert_node(
                    &mut nodes,
                    &mut sources,
                    id,
                    &source,
                    ArtifactNode {
                        id: id.clone(),
                        artifact_type: "code".to_string(),
                        path: None,
                        paths: entry.paths.clone(),
                        owners: entry.owners.clone(),
                        status: None,
                        owns: Some(entry.paths.join(", ")),
                        relations,
                    },
                )?;
            }
        }

        Ok(Self { nodes })
    }

    pub fn impact(&self, changed: &[String]) -> ImpactReport {
        let mut affected: BTreeMap<String, ImpactItem> = BTreeMap::new();
        let changed_set: BTreeSet<&str> = changed.iter().map(String::as_str).collect();

        for changed_id in changed {
            if let Some(changed_node) = self.nodes.get(changed_id) {
                for relation in changed_node
                    .relations
                    .iter()
                    .filter(|r| r.kind == "affects" && !changed_set.contains(r.target.as_str()))
                {
                    self.add_impact(&mut affected, &relation.target, "affects");
                }
            }

            for node in self.nodes.values() {
                if changed_set.contains(node.id.as_str()) {
                    continue;
                }
                for relation in node.relations.iter().filter(|r| &r.target == changed_id) {
                    let via = format!("{}:{}", relation.kind, changed_id);
                    self.add_impact(&mut affected, &node.id, &via);
                }
            }
        }

        ImpactReport {
            changed: changed.to_vec(),
            affected: affected.into_values().collect(),
        }
    }

    fn add_impact(&self, affected: &mut BTreeMap<String, ImpactItem>, id: &str, via: &str) {
        if let Some(node) = self.nodes.get(id) {
            let item = affected
                .entry(id.to_string())
                .or_insert_with(|| ImpactItem {
                    id: node.id.clone(),
                    artifact_type: node.artifact_type.clone(),
                    path: node.path.clone(),
                    paths: node.paths.clone(),
                    owners: node.owners.clone(),
                    via: Vec::new(),
                });
            if !item.via.iter().any(|v| v == via) {
                item.via.push(via.to_string());
            }
        }
    }
}

fn insert_node(
    nodes: &mut BTreeMap<String, ArtifactNode>,
    sources: &mut BTreeMap<String, String>,
    id: &str,
    source: &str,
    node: ArtifactNode,
) -> Result<()> {
    if let Some(previous) = sources.get(id) {
        bail!(
            "Duplicate artifact id '{}' in {} and {}",
            id,
            previous,
            source
        );
    }
    sources.insert(id.to_string(), source.to_string());
    nodes.insert(id.to_string(), node);
    Ok(())
}

fn scan_artifacts_dir(dir: &Path) -> Result<Vec<ArtifactMeta>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().map(|e| e == "md").unwrap_or(false) {
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let kind = classify_artifact(&name);
            result.push(ArtifactMeta { name, path, kind });
        }
    }
    result.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(result)
}

pub fn parse_frontmatter(content: &str) -> Result<Option<ArtifactFrontmatter>> {
    let Some(rest) = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))
    else {
        return Ok(None);
    };
    let Some((yaml, _body)) = rest
        .split_once("\n---")
        .or_else(|| rest.split_once("\r\n---"))
    else {
        return Ok(None);
    };
    if !looks_like_hlv_frontmatter(yaml)? {
        return Ok(None);
    }
    let meta: ArtifactFrontmatter = serde_yaml::from_str(yaml)?;
    Ok(Some(meta))
}

fn looks_like_hlv_frontmatter(yaml: &str) -> Result<bool> {
    let value: serde_yaml::Value = serde_yaml::from_str(yaml)?;
    let Some(mapping) = value.as_mapping() else {
        return Ok(false);
    };
    let has_key = |key: &str| {
        mapping
            .keys()
            .any(|k| k.as_str().map(|s| s == key).unwrap_or(false))
    };
    let relation_keys = [
        "depends_on",
        "affects",
        "requires",
        "implements",
        "verifies",
        "documents",
        "supersedes",
        "conflicts_with",
    ];
    let has_relation = relation_keys.iter().any(|key| has_key(key));
    let has_hlv_identity = has_key("id")
        && (has_key("type") || has_key("owners") || has_key("owns") || has_key("status"));
    Ok(has_hlv_identity || has_relation)
}

fn relations_from_frontmatter(frontmatter: &ArtifactFrontmatter) -> Vec<ArtifactRelation> {
    let mut relations = Vec::new();
    push_relations(&mut relations, "requires", &frontmatter.depends_on);
    push_relations(&mut relations, "requires", &frontmatter.requires);
    push_relations(&mut relations, "implements", &frontmatter.implements);
    push_relations(&mut relations, "verifies", &frontmatter.verifies);
    push_relations(&mut relations, "documents", &frontmatter.documents);
    push_relations(&mut relations, "supersedes", &frontmatter.supersedes);
    push_relations(
        &mut relations,
        "conflicts_with",
        &frontmatter.conflicts_with,
    );
    push_relations(&mut relations, "affects", &frontmatter.affects);
    relations
}

fn push_relations(relations: &mut Vec<ArtifactRelation>, kind: &str, targets: &[String]) {
    for target in targets {
        relations.push(ArtifactRelation {
            kind: kind.to_string(),
            target: target.clone(),
        });
    }
}

fn artifact_markdown_paths(
    root: &Path,
    project: &ProjectMap,
    milestone_id: Option<&str>,
) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    if let Some(global) = project.paths.human.artifacts.as_deref() {
        collect_md_recursive(&root.join(global), &mut paths)?;
    } else {
        collect_md_recursive(&root.join("human/artifacts"), &mut paths)?;
    }
    if let Some(mid) = milestone_id {
        collect_md_recursive(
            &root.join("human/milestones").join(mid).join("artifacts"),
            &mut paths,
        )?;
    }
    paths.sort();
    Ok(paths)
}

fn collect_md_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_md_recursive(&path, out)?;
        } else if path.extension().map(|e| e == "md").unwrap_or(false) {
            out.push(path);
        }
    }
    Ok(())
}

fn classify_artifact(name: &str) -> ArtifactKind {
    match name {
        "context" => ArtifactKind::Context,
        "stack" => ArtifactKind::Stack,
        "constraints" => ArtifactKind::Constraints,
        n if n.starts_with("feature") || n.starts_with("feat") => ArtifactKind::Feature,
        n if n.starts_with("decision") || n.starts_with("adr") => ArtifactKind::Decision,
        _ => ArtifactKind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn classify_known_artifacts() {
        assert_eq!(classify_artifact("context"), ArtifactKind::Context);
        assert_eq!(classify_artifact("stack"), ArtifactKind::Stack);
        assert_eq!(classify_artifact("constraints"), ArtifactKind::Constraints);
        assert_eq!(classify_artifact("feature-auth"), ArtifactKind::Feature);
        assert_eq!(classify_artifact("decision-api"), ArtifactKind::Decision);
        assert_eq!(classify_artifact("something"), ArtifactKind::Unknown);
    }

    #[test]
    fn load_global_artifacts() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let dir = root.join("human/artifacts");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("context.md"), "# Context").unwrap();
        std::fs::write(dir.join("stack.md"), "# Stack").unwrap();
        std::fs::write(dir.join("notes.txt"), "not an artifact").unwrap();

        let artifacts = ArtifactIndex::load_global(root).unwrap();
        assert_eq!(artifacts.len(), 2);
        assert_eq!(artifacts[0].name, "context");
        assert_eq!(artifacts[0].kind, ArtifactKind::Context);
        assert_eq!(artifacts[1].name, "stack");
    }

    #[test]
    fn load_full_artifact() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("context.md");
        std::fs::write(&path, "# Context\n\nSome content").unwrap();

        let full = ArtifactFull::load(&path).unwrap();
        assert_eq!(full.name, "context");
        assert_eq!(full.kind, ArtifactKind::Context);
        assert!(full.content.contains("Some content"));
    }

    #[test]
    fn parse_artifact_frontmatter() {
        let content = r#"---
id: adr-auth-session
type: adr
status: accepted
owners: [platform]
depends_on:
  - spec-auth
affects:
  - code-auth-session
---
# ADR
"#;
        let meta = parse_frontmatter(content).unwrap().unwrap();
        assert_eq!(meta.id, "adr-auth-session");
        assert_eq!(meta.artifact_type, "adr");
        assert_eq!(meta.owners, vec!["platform"]);
        assert_eq!(meta.depends_on, vec!["spec-auth"]);
        assert_eq!(meta.affects, vec!["code-auth-session"]);
    }

    #[test]
    fn parse_artifact_frontmatter_with_crlf() {
        let content = "---\r\nid: spec-checkout\r\ntype: spec\r\nowners: [product]\r\naffects: [code-checkout]\r\n---\r\n# Spec\r\n";
        let meta = parse_frontmatter(content).unwrap().unwrap();
        assert_eq!(meta.id, "spec-checkout");
        assert_eq!(meta.artifact_type, "spec");
        assert_eq!(meta.affects, vec!["code-checkout"]);
    }

    #[test]
    fn legacy_markdown_frontmatter_is_ignored() {
        let content = r#"---
title: Auth Notes
date: 2026-01-01
tags: [auth]
---
# Notes
"#;
        assert!(parse_frontmatter(content).unwrap().is_none());
    }

    #[test]
    fn empty_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let artifacts = ArtifactIndex::load_global(tmp.path()).unwrap();
        assert!(artifacts.is_empty());
    }
}
