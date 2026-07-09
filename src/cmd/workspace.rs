use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::cmd::style;
use crate::mcp::workspace::{WorkspaceConfig, WorkspaceProject};

/// Default workspace config path: ~/.hlv/workspace.yaml
fn default_path() -> Result<PathBuf> {
    let home =
        std::env::var("HOME").with_context(|| "Cannot determine home directory (HOME not set)")?;
    Ok(PathBuf::from(home).join(".hlv").join("workspace.yaml"))
}

/// Resolve workspace path: explicit or default.
fn resolve_path(explicit: Option<&str>) -> Result<PathBuf> {
    match explicit {
        Some(p) => Ok(PathBuf::from(p)),
        None => default_path(),
    }
}

/// `hlv workspace init`
pub fn run_init(path: Option<&str>) -> Result<()> {
    let ws_path = resolve_path(path)?;
    style::header("workspace init");

    if ws_path.exists() {
        style::warn(&format!("Already exists: {}", ws_path.display()));
        style::hint("Use 'hlv workspace add' to add projects");
        return Ok(());
    }

    if let Some(parent) = ws_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Cannot create directory: {}", parent.display()))?;
    }

    let config = WorkspaceConfig {
        projects: Vec::new(),
    };
    let yaml = serde_yaml::to_string(&config)?;
    std::fs::write(&ws_path, yaml)
        .with_context(|| format!("Cannot write: {}", ws_path.display()))?;

    style::ok(&format!("Created {}", ws_path.display()));
    style::hint("Add projects with: hlv workspace add [id]");
    Ok(())
}

/// `hlv workspace add [id] [--root path]`
pub fn run_add(id: Option<&str>, root: Option<&str>, ws_path: Option<&str>) -> Result<()> {
    let ws_file = resolve_path(ws_path)?;
    style::header("workspace add");

    // Resolve project root
    let project_root = match root {
        Some(r) => PathBuf::from(r).canonicalize()?,
        None => std::env::current_dir()?,
    };

    // Validate project root has project.yaml or .hlv/project.yaml
    if !crate::has_project_config(&project_root) {
        anyhow::bail!(
            "No project.yaml or .hlv/project.yaml in {}. Run 'hlv init' first.",
            project_root.display()
        );
    }

    // Derive ID from directory name if not provided
    let project_id = match id {
        Some(i) => i.to_string(),
        None => project_root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or_else(|| anyhow::anyhow!("Cannot derive project ID from path"))?,
    };

    // Load or create workspace config
    let mut config = if ws_file.exists() {
        WorkspaceConfig::load_lenient(&ws_file)?
    } else {
        // Auto-init if not exists
        if let Some(parent) = ws_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        WorkspaceConfig {
            projects: Vec::new(),
        }
    };

    // Check for duplicate ID
    if config.find(&project_id).is_some() {
        anyhow::bail!("Project '{}' already in workspace", project_id);
    }

    // Check for duplicate root
    if config.projects.iter().any(|p| p.root == project_root) {
        anyhow::bail!("Path {} already in workspace", project_root.display());
    }

    config.projects.push(WorkspaceProject {
        id: project_id.clone(),
        root: project_root.clone(),
    });

    let yaml = serde_yaml::to_string(&config)?;
    std::fs::write(&ws_file, yaml)?;

    style::ok(&format!(
        "Added '{}' → {}",
        project_id,
        project_root.display()
    ));
    style::detail("workspace", &ws_file.display().to_string());
    style::detail("projects", &config.projects.len().to_string());
    Ok(())
}

/// `hlv workspace remove <id>`
pub fn run_remove(id: &str, ws_path: Option<&str>) -> Result<()> {
    let ws_file = resolve_path(ws_path)?;
    style::header("workspace remove");

    let mut config = WorkspaceConfig::load_lenient(&ws_file)?;

    let before = config.projects.len();
    config.projects.retain(|p| p.id != id);

    if config.projects.len() == before {
        anyhow::bail!("Project '{}' not found in workspace", id);
    }

    let yaml = serde_yaml::to_string(&config)?;
    std::fs::write(&ws_file, yaml)?;

    style::ok(&format!("Removed '{}'", id));
    style::detail("remaining", &config.projects.len().to_string());
    Ok(())
}

/// `hlv workspace list`
pub fn run_list(ws_path: Option<&str>) -> Result<()> {
    let ws_file = resolve_path(ws_path)?;
    style::header("workspace");

    if !ws_file.exists() {
        style::warn("No workspace config found");
        style::hint(&format!(
            "Create one with: hlv workspace init\n  (default: {})",
            ws_file.display()
        ));
        return Ok(());
    }

    let config = WorkspaceConfig::load_lenient(&ws_file)?;

    style::detail("config", &ws_file.display().to_string());
    println!();

    if config.projects.is_empty() {
        style::hint("No projects. Add with: hlv workspace add");
        return Ok(());
    }

    for p in &config.projects {
        let exists = crate::has_project_config(&p.root);
        let status = if exists { "ok" } else { "missing project.yaml" };
        println!("  • {} → {} ({})", p.id, p.root.display(), status);
    }
    println!();
    Ok(())
}
