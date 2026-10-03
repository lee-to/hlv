use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path, PathBuf};

use anyhow::{ensure, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::Diagnostic;
use crate::model::execution_evidence::{
    ExecutionEvidenceFile, ExecutionOutcome, ExecutionSnapshot,
};
use crate::model::policy::GatesPolicy;
use crate::model::project::{ExecutionEvidenceBinding, ProjectMap};
use crate::model::traceability::TraceabilityMap;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    NotConfigured,
    NotChecked,
    Passed,
    Failed,
    Missing,
    Stale,
    Incomplete,
    Skipped,
    Invalid,
}

impl std::fmt::Display for EvidenceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NotConfigured => "not_configured",
            Self::NotChecked => "not_checked",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Missing => "missing",
            Self::Stale => "stale",
            Self::Incomplete => "incomplete",
            Self::Skipped => "skipped",
            Self::Invalid => "invalid",
        })
    }
}

pub fn check_configuration(config: &crate::model::project::ExecutionEvidenceConfig) -> Result<()> {
    validate_relative(&config.path)?;
    ensure!(
        !config.bindings.is_empty(),
        "execution_evidence.bindings must not be empty"
    );
    let mut seen = HashSet::new();
    for binding in &config.bindings {
        ensure!(
            !binding.id.trim().is_empty() && seen.insert(&binding.id),
            "empty or duplicate execution evidence binding ID"
        );
        validate_binding(binding)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct BindingEvidenceStatus {
    pub binding: String,
    pub requirement: String,
    pub test: String,
    pub gate: String,
    pub status: EvidenceStatus,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecutionEvidenceReport {
    pub status: EvidenceStatus,
    pub bindings: Vec<BindingEvidenceStatus>,
}

/// Validate planned binding prerequisites without requiring code, test files or runs.
/// Used before implementation and before capturing an external runner's snapshot.
pub fn check_execution_prerequisites(
    repo_root: &Path,
    project: &ProjectMap,
) -> (ExecutionEvidenceReport, Vec<Diagnostic>) {
    let Some(config) = &project.execution_evidence else {
        return (
            ExecutionEvidenceReport {
                status: EvidenceStatus::NotConfigured,
                bindings: vec![],
            },
            vec![],
        );
    };
    if let Err(error) = check_configuration(config) {
        return (
            ExecutionEvidenceReport {
                status: EvidenceStatus::Invalid,
                bindings: vec![],
            },
            vec![Diagnostic::error(
                "EVD-010",
                format!("Invalid evidence configuration: {error}"),
            )
            .with_file("project.yaml")],
        );
    }
    let mut bindings = Vec::new();
    let mut diagnostics = Vec::new();
    for binding in &config.bindings {
        let (status, reason) = match resolve_binding(repo_root, project, binding) {
            Ok(_) => (
                EvidenceStatus::NotChecked,
                "execution outcomes deferred until after runner execution".to_string(),
            ),
            Err(error) => {
                diagnostics.push(
                    Diagnostic::error("EVD-010", format!("Binding {}: {error}", binding.id))
                        .with_file("project.yaml"),
                );
                (EvidenceStatus::Invalid, error.to_string())
            }
        };
        bindings.push(BindingEvidenceStatus {
            binding: binding.id.clone(),
            requirement: binding.requirement.clone(),
            test: binding.test.clone(),
            gate: binding.gate.clone(),
            status,
            reason,
            run_id: None,
            code_revision: None,
            artifact_ref: None,
        });
    }
    let status = if diagnostics.is_empty() {
        EvidenceStatus::NotChecked
    } else {
        EvidenceStatus::Invalid
    };
    (ExecutionEvidenceReport { status, bindings }, diagnostics)
}

/// Check evidence without running tests or mutating artifacts.
pub fn check_execution_evidence(
    repo_root: &Path,
    project: &ProjectMap,
) -> (ExecutionEvidenceReport, Vec<Diagnostic>) {
    let context = crate::ProjectContext::from_root(repo_root);
    let repo_root = context.repo_root();
    let Some(config) = &project.execution_evidence else {
        return (
            ExecutionEvidenceReport {
                status: EvidenceStatus::NotConfigured,
                bindings: vec![],
            },
            vec![],
        );
    };
    let config_root = crate::config_root(repo_root);
    let mut diagnostics = Vec::new();
    let evidence = (|| -> Result<Option<ExecutionEvidenceFile>> {
        validate_relative(&config.path)?;
        let path = config_root.join(&config.path);
        if !path.try_exists()? {
            return Ok(None);
        }
        let path = contained_path(&config_root, &config.path)?;
        let file = ExecutionEvidenceFile::load(&path)?;
        ensure!(
            file.schema_version == 1,
            "unsupported evidence schema_version"
        );
        let mut seen = HashSet::new();
        for run in &file.runs {
            ensure!(
                seen.insert(&run.snapshot.binding),
                "duplicate evidence binding {}",
                run.snapshot.binding
            );
            ensure!(
                config.bindings.iter().any(|b| b.id == run.snapshot.binding),
                "unknown evidence binding {}",
                run.snapshot.binding
            );
        }
        Ok(Some(file))
    })();
    if let Err(error) = &evidence {
        diagnostics.push(
            Diagnostic::error(
                "EVD-001",
                format!("Cannot consume execution evidence: {error}"),
            )
            .with_file(&config.path),
        );
    }
    if config.bindings.is_empty() {
        diagnostics.push(
            Diagnostic::error("EVD-010", "execution_evidence.bindings must not be empty")
                .with_file("project.yaml"),
        );
    }
    let mut seen = HashSet::new();
    let mut bindings = Vec::new();
    for binding in &config.bindings {
        let mut result = BindingEvidenceStatus {
            binding: binding.id.clone(),
            requirement: binding.requirement.clone(),
            test: binding.test.clone(),
            gate: binding.gate.clone(),
            status: EvidenceStatus::Invalid,
            reason: String::new(),
            run_id: None,
            code_revision: None,
            artifact_ref: None,
        };
        let snapshot = if seen.insert(&binding.id) {
            capture_snapshot(repo_root, project, binding)
        } else {
            Err(anyhow::anyhow!("duplicate binding ID {}", binding.id))
        };
        match snapshot {
            Err(error) => {
                result.reason = error.to_string();
                diagnostics.push(
                    Diagnostic::error("EVD-010", format!("Binding {}: {error}", binding.id))
                        .with_file("project.yaml"),
                );
            }
            Ok(snapshot) => match &evidence {
                Err(error) => result.reason = error.to_string(),
                Ok(file) => {
                    let run = file
                        .as_ref()
                        .and_then(|f| f.runs.iter().find(|r| r.snapshot.binding == binding.id));
                    if let Some(run) = run {
                        result.run_id = Some(run.run_id.clone());
                        result.code_revision = Some(run.code_revision.clone());
                        result.artifact_ref = run.artifact_ref.clone();
                        if run.run_id.trim().is_empty()
                            || run.code_revision.trim().is_empty()
                            || run.snapshot.inputs.values().any(|hash| {
                                hash.len() != 64
                                    || !hash
                                        .bytes()
                                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                            })
                            || (run.outcome != ExecutionOutcome::Incomplete
                                && (run.finished_at.is_none()
                                    || run
                                        .artifact_ref
                                        .as_ref()
                                        .is_none_or(|s| s.trim().is_empty())))
                        {
                            result.reason = "run identity, terminal timestamp, artifact reference or SHA-256 hashes are invalid".into();
                        } else if run.snapshot != snapshot {
                            result.status = EvidenceStatus::Stale;
                            result.reason = "requirement approval, test/gate identity or input snapshot changed".into();
                        } else {
                            result.status = match run.outcome {
                                ExecutionOutcome::Passed => EvidenceStatus::Passed,
                                ExecutionOutcome::Failed => EvidenceStatus::Failed,
                                ExecutionOutcome::Incomplete => EvidenceStatus::Incomplete,
                                ExecutionOutcome::Skipped => EvidenceStatus::Skipped,
                            };
                            result.reason = format!(
                                "external runner reported {} for compatible inputs",
                                result.status
                            );
                        }
                    } else {
                        result.status = EvidenceStatus::Missing;
                        result.reason = "no external run supplied for this binding".into();
                    }
                    let code = match result.status {
                        EvidenceStatus::Passed => None,
                        EvidenceStatus::Missing => Some("EVD-020"),
                        EvidenceStatus::Stale => Some("EVD-030"),
                        EvidenceStatus::Invalid => Some("EVD-001"),
                        _ => Some("EVD-040"),
                    };
                    if let Some(code) = code {
                        diagnostics.push(
                            Diagnostic::error(
                                code,
                                format!("Binding {}: {}", binding.id, result.reason),
                            )
                            .with_file(&config.path),
                        );
                    }
                }
            },
        }
        bindings.push(result);
    }
    // Preserve the first actionable failure; never infer a pass from an empty set.
    let status = bindings
        .iter()
        .find(|b| b.status != EvidenceStatus::Passed)
        .map(|b| b.status.clone())
        .unwrap_or_else(|| {
            if bindings.is_empty() {
                EvidenceStatus::Invalid
            } else {
                EvidenceStatus::Passed
            }
        });
    (ExecutionEvidenceReport { status, bindings }, diagnostics)
}

/// Capture identities and exact regular-file content, including dirty/untracked files.
/// Directories recurse, so adding, deleting or renaming a file invalidates evidence.
pub fn capture_snapshot(
    repo_root: &Path,
    project: &ProjectMap,
    binding: &ExecutionEvidenceBinding,
) -> Result<ExecutionSnapshot> {
    let context = crate::ProjectContext::from_root(repo_root);
    let repo_root = context.repo_root();
    let (_, gates_path) = resolve_binding(repo_root, project, binding)?;
    let root = repo_root.canonicalize()?;
    let mut inputs = BTreeMap::new();
    hash_path(&root, &gates_path, &mut inputs)?;
    for path in std::iter::once(&binding.requirement_file)
        .chain(&binding.test_paths)
        .chain(&binding.code_paths)
    {
        let full_path = contained_path(&root, path)?;
        let mut files = BTreeMap::new();
        hash_path(&root, &full_path, &mut files)?;
        ensure!(!files.is_empty(), "input directory {path} is empty");
        inputs.extend(files);
    }
    Ok(ExecutionSnapshot {
        binding: binding.id.clone(),
        requirement: binding.requirement.clone(),
        approved_requirement_revision: binding.approved_requirement_revision.clone(),
        test: binding.test.clone(),
        gate: binding.gate.clone(),
        inputs,
    })
}

fn validate_binding(binding: &ExecutionEvidenceBinding) -> Result<()> {
    for value in [
        &binding.id,
        &binding.requirement,
        &binding.approved_requirement_revision,
        &binding.test,
        &binding.gate,
    ] {
        ensure!(
            !value.trim().is_empty(),
            "binding identities and approved revision must not be empty"
        );
    }
    ensure!(
        !binding.test_paths.is_empty() && !binding.code_paths.is_empty(),
        "test_paths and code_paths must not be empty"
    );
    for path in std::iter::once(&binding.requirement_file)
        .chain(&binding.test_paths)
        .chain(&binding.code_paths)
    {
        validate_relative(path)?;
    }
    Ok(())
}

fn resolve_binding(
    repo_root: &Path,
    project: &ProjectMap,
    binding: &ExecutionEvidenceBinding,
) -> Result<(PathBuf, PathBuf)> {
    let context = crate::ProjectContext::from_root(repo_root);
    let repo_root = context.repo_root();
    validate_binding(binding)?;
    let requirement_file = contained_path(repo_root, &binding.requirement_file)?;
    let trace = TraceabilityMap::load(&requirement_file)
        .context("cannot load requirement traceability file")?;
    ensure!(
        trace
            .requirements
            .iter()
            .any(|r| r.id == binding.requirement),
        "unknown requirement {}",
        binding.requirement
    );
    ensure!(
        trace
            .mappings
            .iter()
            .any(|m| m.requirement == binding.requirement
                && m.tests.contains(&binding.test)
                && m.runtime_gates.contains(&binding.gate)),
        "binding is not a requirement/test/gate mapping in traceability"
    );
    let gates_path = contained_path(
        &crate::config_root(repo_root).canonicalize()?,
        &project.paths.validation.gates_policy,
    )?;
    let gates = GatesPolicy::load(&gates_path)?;
    ensure!(
        gates.gates.iter().any(|g| g.id == binding.gate),
        "unknown gate {}",
        binding.gate
    );
    Ok((requirement_file, gates_path))
}

fn validate_relative(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty()
            && Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "path must be relative without '.' or '..': {path}"
    );
    Ok(())
}

fn contained_path(root: &Path, path: &str) -> Result<PathBuf> {
    validate_relative(path)?;
    let full = root.join(path);
    let canonical = full
        .canonicalize()
        .with_context(|| format!("cannot resolve input {path}"))?;
    ensure!(
        canonical.starts_with(root.canonicalize()?),
        "path escapes project root: {path}"
    );
    // Reject symlinks, including intermediate components, so snapshot keys are unambiguous.
    let mut cursor = root.to_path_buf();
    for component in Path::new(path).components() {
        cursor.push(component);
        ensure!(
            !std::fs::symlink_metadata(&cursor)?.file_type().is_symlink(),
            "symlink input is unsupported: {path}"
        );
    }
    Ok(full)
}

fn hash_path(root: &Path, path: &Path, inputs: &mut BTreeMap<String, String>) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "symlink input is unsupported: {}",
        path.display()
    );
    if metadata.is_dir() {
        for entry in std::fs::read_dir(path)? {
            hash_path(root, &entry?.path(), inputs)?;
        }
    } else {
        ensure!(
            metadata.is_file(),
            "input is not a regular file: {}",
            path.display()
        );
        let relative = path
            .strip_prefix(root)?
            .to_str()
            .context("input path is not UTF-8")?
            .replace('\\', "/");
        inputs.insert(
            relative,
            format!("{:x}", Sha256::digest(std::fs::read(path)?)),
        );
    }
    Ok(())
}
