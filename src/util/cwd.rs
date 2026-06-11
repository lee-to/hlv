use std::path::{Component, Path, PathBuf};

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
    if let Some(cwd) = cwd {
        ensure_project_relative_cwd(cwd, subject)?;
    }
    let (path, label) = resolve_cwd(project_root, cwd);
    if !path.is_dir() {
        anyhow::bail!("{} does not exist: {}", subject, label);
    }
    Ok((path, label))
}

fn ensure_project_relative_cwd(cwd: &str, subject: &str) -> Result<()> {
    let path = Path::new(cwd);
    if path.components().any(|component| {
        matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::ParentDir
        )
    }) {
        anyhow::bail!(
            "{} must be relative to project root and must not contain '..': {}",
            subject,
            cwd
        );
    }
    Ok(())
}
