pub mod check;
pub mod cmd;
pub mod mcp;
pub mod model;
pub mod parse;
pub mod tui;
pub mod util;

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// True when `dir` is an HLV project root: either a greenfield root with
/// `project.yaml` or an adopted root with `.hlv/project.yaml`.
pub fn has_project_config(dir: &Path) -> bool {
    dir.join("project.yaml").exists() || dir.join(".hlv").join("project.yaml").exists()
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
        tracing::debug!(root = %path.display(), "project root from explicit --root");
        return Ok(path);
    }

    let start = std::env::current_dir().context("cannot get current directory")?;
    find_project_root_from(&start)
}

/// Search upward from `start` for a directory containing `project.yaml`
/// or `.hlv/project.yaml`. A root-level `project.yaml` in the same directory
/// takes priority over `.hlv/project.yaml` (see [`config_root`]).
pub fn find_project_root_from(start: &Path) -> Result<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if has_project_config(&dir) {
            tracing::debug!(root = %dir.display(), "project root found");
            return Ok(dir);
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
    let config = if root.join("project.yaml").exists() {
        root.to_path_buf()
    } else if root.join(".hlv").join("project.yaml").exists() {
        root.join(".hlv")
    } else {
        root.to_path_buf()
    };
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
