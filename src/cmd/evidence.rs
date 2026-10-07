use std::path::Path;

use anyhow::{Context, Result};

use crate::check::execution_evidence::capture_snapshot;
use crate::model::project::ProjectMap;

/// Export a pre-run snapshot for an existing external runner; no tests are executed.
pub fn run_snapshot(root: &Path) -> Result<()> {
    let project = ProjectMap::load(&crate::config_root(root).join("project.yaml"))?;
    let config = project
        .execution_evidence
        .as_ref()
        .context("execution_evidence is not configured")?;
    crate::check::execution_evidence::check_configuration(config)?;
    let mut snapshots = Vec::new();
    for binding in &config.bindings {
        snapshots.push(capture_snapshot(root, &project, binding)?);
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"schema_version": 1, "snapshots": snapshots})
        )?
    );
    Ok(())
}
