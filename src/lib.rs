pub mod check;
pub mod cmd;
pub mod mcp;
pub mod model;
pub mod parse;
pub mod tui;

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Find the project root by searching upward for `project.yaml`.
/// If `explicit` is provided, use that path directly.
pub fn find_project_root(explicit: Option<&str>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        let path = PathBuf::from(p);
        anyhow::ensure!(
            path.join("project.yaml").exists(),
            "No project.yaml found at {}",
            path.display()
        );
        return Ok(path);
    }

    let start = std::env::current_dir().context("cannot get current directory")?;
    find_project_root_from(&start)
}

/// Search upward from `start` for a directory containing `project.yaml`.
pub fn find_project_root_from(start: &Path) -> Result<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if dir.join("project.yaml").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            anyhow::bail!(
                "No project.yaml found in any parent directory. \
                 Use --project or run from inside an HLV project."
            );
        }
    }
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
    fn find_project_root_upward_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let subdir = tmp.path().join("no_project_here");
        std::fs::create_dir_all(&subdir).unwrap();

        let result = find_project_root_from(&subdir);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No project.yaml"));
    }
}
