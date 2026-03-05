//! Routing logic for single-project vs workspace (multi-project) MCP modes.
//!
//! In single-project mode, all resources/tools operate on a fixed project root.
//! In workspace mode, URIs are prefixed with `hlv://projects/{id}/...` and
//! tools receive an extra `project_id` parameter.

use rmcp::ErrorData as McpError;
use std::path::PathBuf;

use super::workspace::WorkspaceConfig;

/// Server operating mode: single project or workspace (multi-project).
#[derive(Debug, Clone)]
pub enum ServerMode {
    /// Traditional single-project mode with a fixed root.
    Single(PathBuf),
    /// Multi-project workspace mode.
    Workspace(WorkspaceConfig),
}

impl ServerMode {
    /// Whether this server is running in workspace mode.
    pub fn is_workspace(&self) -> bool {
        matches!(self, Self::Workspace(_))
    }

    /// Resolve the project root for a given `project_id`.
    ///
    /// - In single mode: `project_id` is ignored, always returns the fixed root.
    /// - In workspace mode: `project_id` is required and must match a known project.
    pub fn resolve_root(&self, project_id: Option<&str>) -> Result<PathBuf, McpError> {
        match self {
            Self::Single(root) => Ok(root.clone()),
            Self::Workspace(config) => {
                let id = project_id.ok_or_else(|| {
                    McpError::invalid_params(
                        "project_id is required in workspace mode".to_string(),
                        None,
                    )
                })?;
                config.find(id).map(|p| p.root.clone()).ok_or_else(|| {
                    McpError::invalid_params(format!("Unknown project: '{id}'"), None)
                })
            }
        }
    }

    /// Get workspace config (only available in workspace mode).
    pub fn workspace(&self) -> Option<&WorkspaceConfig> {
        match self {
            Self::Workspace(config) => Some(config),
            _ => None,
        }
    }
}

/// Parse a workspace-prefixed URI, returning `(project_id, inner_uri)`.
///
/// Example: `"hlv://projects/backend/milestones"` → `Some(("backend", "hlv://milestones"))`
///
/// Returns `None` if the URI doesn't match the workspace pattern.
pub fn parse_workspace_uri(uri: &str) -> Option<(&str, String)> {
    let rest = uri.strip_prefix("hlv://projects/")?;
    let (project_id, path) = rest.split_once('/').unwrap_or((rest, ""));
    if project_id.is_empty() {
        return None;
    }
    if path.is_empty() {
        // hlv://projects/{id} → hlv://project (single project info)
        Some((project_id, "hlv://project".to_string()))
    } else {
        Some((project_id, format!("hlv://{path}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_workspace_uri_milestones() {
        let (id, inner) = parse_workspace_uri("hlv://projects/backend/milestones").unwrap();
        assert_eq!(id, "backend");
        assert_eq!(inner, "hlv://milestones");
    }

    #[test]
    fn parse_workspace_uri_stage() {
        let (id, inner) = parse_workspace_uri("hlv://projects/api/stage/1").unwrap();
        assert_eq!(id, "api");
        assert_eq!(inner, "hlv://stage/1");
    }

    #[test]
    fn parse_workspace_uri_project_shorthand() {
        let (id, inner) = parse_workspace_uri("hlv://projects/backend").unwrap();
        assert_eq!(id, "backend");
        assert_eq!(inner, "hlv://project");
    }

    #[test]
    fn parse_workspace_uri_not_workspace() {
        assert!(parse_workspace_uri("hlv://milestones").is_none());
        assert!(parse_workspace_uri("hlv://project").is_none());
    }

    #[test]
    fn single_mode_ignores_project_id() {
        let mode = ServerMode::Single(PathBuf::from("/test"));
        assert_eq!(mode.resolve_root(None).unwrap(), PathBuf::from("/test"));
        assert_eq!(
            mode.resolve_root(Some("ignored")).unwrap(),
            PathBuf::from("/test")
        );
    }

    #[test]
    fn workspace_mode_requires_project_id() {
        use crate::mcp::workspace::{WorkspaceConfig, WorkspaceProject};
        let mode = ServerMode::Workspace(WorkspaceConfig {
            projects: vec![WorkspaceProject {
                id: "api".to_string(),
                root: PathBuf::from("/api"),
            }],
        });

        assert!(mode.resolve_root(None).is_err());
        assert_eq!(
            mode.resolve_root(Some("api")).unwrap(),
            PathBuf::from("/api")
        );
        assert!(mode.resolve_root(Some("unknown")).is_err());
    }
}
