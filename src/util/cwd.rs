use std::path::{Path, PathBuf};

use anyhow::Result;

pub fn resolve_cwd(project_root: &Path, cwd: Option<&str>) -> (PathBuf, String) {
    let label = cwd.unwrap_or(".").to_string();
    let path = match cwd {
        Some(rel) => project_root.join(rel),
        None => project_root.to_path_buf(),
    };
    (path, label)
}

pub fn ensure_existing_cwd(
    project_root: &Path,
    cwd: Option<&str>,
    subject: &str,
) -> Result<(PathBuf, String)> {
    let (path, label) = resolve_cwd(project_root, cwd);
    if !path.is_dir() {
        anyhow::bail!("{} does not exist: {}", subject, label);
    }
    Ok((path, label))
}
