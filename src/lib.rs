pub mod check;
pub mod cmd;
pub mod index;
pub mod mcp;
pub mod model;
pub mod parse;
pub mod tui;
pub mod util;

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Project layout discovered from the filesystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectLayout {
    /// HLV-owned artifacts live at the repository root.
    Greenfield,
    /// HLV-owned artifacts live under `.hlv/`; repository code stays at root.
    Adopted,
}

/// Resolved project roots for commands that need to distinguish repo-owned
/// paths from HLV-owned configuration paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectContext {
    repo_root: PathBuf,
    hlv_root: PathBuf,
    layout: ProjectLayout,
}

impl ProjectContext {
    pub fn greenfield(root: &Path) -> Self {
        Self {
            repo_root: root.to_path_buf(),
            hlv_root: root.to_path_buf(),
            layout: ProjectLayout::Greenfield,
        }
    }

    pub fn adopted(root: &Path) -> Self {
        Self {
            repo_root: root.to_path_buf(),
            hlv_root: root.join(".hlv"),
            layout: ProjectLayout::Adopted,
        }
    }

    pub fn from_root(root: &Path) -> Self {
        let root = normalize_project_root(root);
        if root.join("project.yaml").exists() {
            Self::greenfield(&root)
        } else if root.join(".hlv").join("project.yaml").exists() {
            Self::adopted(&root)
        } else {
            Self::greenfield(&root)
        }
    }

    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    pub fn hlv_root(&self) -> &Path {
        &self.hlv_root
    }

    pub fn layout(&self) -> ProjectLayout {
        self.layout
    }

    pub fn is_adopted(&self) -> bool {
        self.layout == ProjectLayout::Adopted
    }

    /// Resolve a repository-owned path such as observed source code or git data.
    pub fn repo_path(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.repo_root.join(relative)
    }

    /// Resolve an HLV-owned artifact path such as project.yaml, human/, or validation/.
    pub fn hlv_path(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.hlv_root.join(relative)
    }

    /// Resolve the configured generated source root. In adopted projects this
    /// remains under the HLV config root for compatibility with the existing
    /// `paths.llm` contract.
    pub fn generated_code_path(
        &self,
        project: &crate::model::project::ProjectMap,
    ) -> Option<PathBuf> {
        project.paths.llm.src.as_ref().map(|p| self.hlv_path(p))
    }

    /// Resolve the configured generated test root, if present.
    pub fn generated_tests_path(
        &self,
        project: &crate::model::project::ProjectMap,
    ) -> Option<PathBuf> {
        project.paths.llm.tests.as_ref().map(|p| self.hlv_path(p))
    }
}

/// True when `dir` is an HLV project root: either a greenfield root with
/// `project.yaml` or an adopted root with `.hlv/project.yaml`.
pub fn has_project_config(dir: &Path) -> bool {
    dir.join("project.yaml").exists() || dir.join(".hlv").join("project.yaml").exists()
}

fn normalize_project_root(root: &Path) -> PathBuf {
    if root.file_name().and_then(|name| name.to_str()) == Some(".hlv")
        && root.join("project.yaml").exists()
    {
        return root
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
    }

    root.to_path_buf()
}

/// Find the project root by searching upward for `project.yaml` or `.hlv/project.yaml`.
/// If `explicit` is provided, use that path directly.
pub fn find_project_root(explicit: Option<&str>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        let path = PathBuf::from(p);
        anyhow::ensure!(
            has_project_config(&path),
            "No project.yaml or .hlv/project.yaml found at {}",
            path.display()
        );
        let root = normalize_project_root(&path);
        tracing::debug!(root = %root.display(), "project root from explicit --root");
        return Ok(root);
    }

    let start = std::env::current_dir().context("cannot get current directory")?;
    find_project_root_from(&start)
}

/// Find and resolve the full project context.
pub fn find_project_context(explicit: Option<&str>) -> Result<ProjectContext> {
    let root = find_project_root(explicit)?;
    Ok(ProjectContext::from_root(&root))
}

/// Search upward from `start` for a directory containing `project.yaml`
/// or `.hlv/project.yaml`. A root-level `project.yaml` in the same directory
/// takes priority over `.hlv/project.yaml` (see [`config_root`]).
pub fn find_project_root_from(start: &Path) -> Result<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if has_project_config(&dir) {
            let root = normalize_project_root(&dir);
            tracing::debug!(root = %root.display(), "project root found");
            return Ok(root);
        }
        if !dir.pop() {
            anyhow::bail!(
                "No project.yaml or .hlv/project.yaml found in any parent directory. \
                 Use --root or run from inside an HLV project."
            );
        }
    }
}

/// Resolve the HLV config root for a project root.
///
/// Greenfield projects keep HLV artifacts at the repository root; adopted
/// projects keep them under `.hlv/`. A root-level `project.yaml` always
/// takes priority when both layouts are present.
pub fn config_root(root: &Path) -> PathBuf {
    let config = ProjectContext::from_root(root).hlv_root().to_path_buf();
    tracing::debug!(root = %root.display(), config_root = %config.display(), "config root resolved");
    config
}

/// Resolve a relative path against the project root.
pub fn resolve_path(root: &Path, relative: &str) -> PathBuf {
    root.join(relative)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_path_joins() {
        let root = Path::new("/projects/myapp");
        let result = resolve_path(root, "human/milestones/001/contracts/order.md");
        assert_eq!(
            result,
            PathBuf::from("/projects/myapp/human/milestones/001/contracts/order.md")
        );
    }

    #[test]
    fn find_project_root_explicit_valid() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("project.yaml"), "project: test").unwrap();
        let result = find_project_root(Some(tmp.path().to_str().unwrap()));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), tmp.path());
    }

    #[test]
    fn find_project_root_explicit_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let result = find_project_root(Some(tmp.path().to_str().unwrap()));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No project.yaml"));
    }

    #[test]
    fn find_project_root_upward_search() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        std::fs::write(root.join("project.yaml"), "project: test").unwrap();
        let subdir = root.join("a/b/c");
        std::fs::create_dir_all(&subdir).unwrap();

        let result = find_project_root_from(&subdir);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().canonicalize().unwrap(), root);
    }

    #[test]
    fn config_root_prefers_root_project_yaml() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("project.yaml"), "project: test").unwrap();
        std::fs::create_dir_all(tmp.path().join(".hlv")).unwrap();
        std::fs::write(tmp.path().join(".hlv/project.yaml"), "project: adopted").unwrap();
        assert_eq!(config_root(tmp.path()), tmp.path());
    }

    #[test]
    fn config_root_uses_hlv_dir_for_adopted() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".hlv")).unwrap();
        std::fs::write(tmp.path().join(".hlv/project.yaml"), "project: adopted").unwrap();
        assert_eq!(config_root(tmp.path()), tmp.path().join(".hlv"));
    }

    #[test]
    fn project_context_resolves_repo_and_hlv_paths() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".hlv")).unwrap();
        std::fs::write(tmp.path().join(".hlv/project.yaml"), "project: adopted").unwrap();

        let context = ProjectContext::from_root(tmp.path());
        assert!(context.is_adopted());
        assert_eq!(
            context.repo_path("app/User.php"),
            tmp.path().join("app/User.php")
        );
        assert_eq!(
            context.hlv_path("validation/gates-policy.yaml"),
            tmp.path().join(".hlv/validation/gates-policy.yaml")
        );
    }

    #[test]
    fn project_context_resolves_generated_roots_under_hlv_root() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".hlv")).unwrap();
        std::fs::write(tmp.path().join(".hlv/project.yaml"), "project: adopted").unwrap();
        let context = ProjectContext::from_root(tmp.path());
        let project: crate::model::project::ProjectMap = serde_yaml::from_str(
            r#"
schema_version: 1
project: adopted
paths:
  human:
    glossary: human/glossary.yaml
    constraints: human/constraints/
  validation:
    gates_policy: validation/gates-policy.yaml
    scenarios: validation/scenarios/
  llm:
    src: llm/src/
    tests: llm/tests/
"#,
        )
        .unwrap();

        assert_eq!(
            context.generated_code_path(&project).unwrap(),
            tmp.path().join(".hlv/llm/src/")
        );
        assert_eq!(
            context.generated_tests_path(&project).unwrap(),
            tmp.path().join(".hlv/llm/tests/")
        );
    }

    #[test]
    fn find_project_root_hlv_discovery_from_nested() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join(".hlv")).unwrap();
        std::fs::write(root.join(".hlv/project.yaml"), "project: adopted").unwrap();
        let subdir = root.join("app/Http/Controllers");
        std::fs::create_dir_all(&subdir).unwrap();

        let result = find_project_root_from(&subdir);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().canonicalize().unwrap(), root);
    }

    #[test]
    fn find_project_root_explicit_hlv_layout() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".hlv")).unwrap();
        std::fs::write(tmp.path().join(".hlv/project.yaml"), "project: adopted").unwrap();
        let result = find_project_root(Some(tmp.path().to_str().unwrap()));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), tmp.path());
    }

    #[test]
    fn find_project_root_upward_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let subdir = tmp.path().join("no_project_here");
        std::fs::create_dir_all(&subdir).unwrap();

        let result = find_project_root_from(&subdir);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No project.yaml"));
        assert!(err.contains("--root"));
    }
}
