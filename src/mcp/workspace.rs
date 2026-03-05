//! Workspace configuration for multi-project MCP mode.
//!
//! A workspace allows a single MCP server to manage multiple HLV projects.
//! Projects are defined in a YAML file (e.g. `~/.hlv/workspace.yaml`).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Workspace configuration listing multiple HLV projects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub projects: Vec<WorkspaceProject>,
}

/// A single project entry in a workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceProject {
    /// Unique identifier for the project (used in URIs and tool params).
    pub id: String,
    /// Absolute path to the project root (must contain `project.yaml`).
    pub root: PathBuf,
}

/// Summary of a project for the `hlv://projects` resource.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ProjectSummary {
    pub id: String,
    pub root: String,
    pub name: Option<String>,
    pub current_milestone: Option<String>,
    pub milestone_status: Option<String>,
    pub stages_total: usize,
    pub stages_done: usize,
}

impl WorkspaceConfig {
    /// Load workspace configuration from a YAML file.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read workspace config: {}", path.display()))?;
        let config: Self = serde_yaml::from_str(&text)
            .with_context(|| format!("Invalid workspace YAML: {}", path.display()))?;
        config.validate()?;
        Ok(config)
    }

    /// Validate that workspace config is well-formed.
    fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            !self.projects.is_empty(),
            "Workspace must contain at least one project"
        );

        // Check for duplicate IDs
        let mut seen = std::collections::HashSet::new();
        for p in &self.projects {
            anyhow::ensure!(!p.id.is_empty(), "Project ID must not be empty");
            anyhow::ensure!(
                seen.insert(&p.id),
                "Duplicate project ID in workspace: '{}'",
                p.id
            );
        }

        // Check that all roots exist and contain project.yaml
        for p in &self.projects {
            anyhow::ensure!(
                p.root.join("project.yaml").exists(),
                "Project '{}': no project.yaml at {}",
                p.id,
                p.root.display()
            );
        }

        Ok(())
    }

    /// Find a project by ID.
    pub fn find(&self, id: &str) -> Option<&WorkspaceProject> {
        self.projects.iter().find(|p| p.id == id)
    }

    /// Build summaries for all projects (for `hlv://projects` resource).
    pub fn summaries(&self) -> Vec<ProjectSummary> {
        self.projects
            .iter()
            .map(|p| {
                let mut summary = load_project_summary(&p.root).unwrap_or_default();
                summary.id = p.id.clone();
                summary.root = p.root.display().to_string();
                summary
            })
            .collect()
    }
}

/// Load summary data from a project root into a ProjectSummary (without id/root).
fn load_project_summary(root: &Path) -> Result<ProjectSummary> {
    let pm = crate::model::project::ProjectMap::load(&root.join("project.yaml"))?;
    let name = Some(pm.project.clone());

    let mm = crate::model::milestone::MilestoneMap::load(&root.join("milestones.yaml"))?;
    match &mm.current {
        Some(current) => {
            let milestone_id = Some(current.id.clone());
            let total = current.stages.len();
            let done = current
                .stages
                .iter()
                .filter(|s| matches!(s.status, crate::model::milestone::StageStatus::Validated))
                .count();
            let status = current
                .stage
                .and_then(|stage_id| current.stages.iter().find(|s| s.id == stage_id))
                .or_else(|| current.stages.last())
                .map(|stage| format!("{:?}", stage.status));
            Ok(ProjectSummary {
                id: String::new(),
                root: String::new(),
                name,
                current_milestone: milestone_id,
                milestone_status: status,
                stages_total: total,
                stages_done: done,
            })
        }
        None => Ok(ProjectSummary {
            id: String::new(),
            root: String::new(),
            name,
            current_milestone: None,
            milestone_status: None,
            stages_total: 0,
            stages_done: 0,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_workspace_yaml() {
        let yaml = r#"
projects:
  - id: backend
    root: /tmp/fake/backend
  - id: frontend
    root: /tmp/fake/frontend
"#;
        let config: WorkspaceConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.projects.len(), 2);
        assert_eq!(config.projects[0].id, "backend");
        assert_eq!(config.projects[1].id, "frontend");
    }

    #[test]
    fn find_project() {
        let config = WorkspaceConfig {
            projects: vec![
                WorkspaceProject {
                    id: "a".to_string(),
                    root: PathBuf::from("/a"),
                },
                WorkspaceProject {
                    id: "b".to_string(),
                    root: PathBuf::from("/b"),
                },
            ],
        };
        assert!(config.find("a").is_some());
        assert!(config.find("c").is_none());
    }
}
