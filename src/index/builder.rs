use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::index::extract_symbols_from_source;
use crate::model::index::{Index, Symbol};
use crate::model::project::ProjectMap;

const IGNORED_DIRS: &[&str] = &[
    ".git",
    ".hlv",
    "target",
    "node_modules",
    "vendor",
    ".venv",
    "dist",
    "build",
    "__pycache__",
    ".pytest_cache",
    ".ruff_cache",
    ".mypy_cache",
];

#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexBuildSummary {
    pub output: PathBuf,
    pub files_scanned: usize,
    pub symbols_indexed: usize,
}

pub fn build_index(project_root: &Path) -> Result<IndexBuildSummary> {
    let context = crate::ProjectContext::from_root(project_root);
    let config_root = context.hlv_root();
    let project = ProjectMap::load(&config_root.join("project.yaml"))?;

    let scan_roots = scan_roots(&context, &project);
    let mut files_scanned = 0usize;
    let mut symbols = Vec::new();

    for root in scan_roots {
        scan_source_root(&root, context.repo_root(), &mut files_scanned, &mut symbols)?;
    }

    symbols.sort_by(|a, b| a.id.cmp(&b.id));

    let index = Index {
        schema_version: 1,
        generated_at: Some(chrono::Utc::now().to_rfc3339()),
        project: Some(project.project),
        symbols,
    };

    let output = config_root.join("index/signatures.yaml");
    write_index_atomic(&output, &index)?;

    Ok(IndexBuildSummary {
        output,
        files_scanned,
        symbols_indexed: index.symbols.len(),
    })
}

fn scan_roots(context: &crate::ProjectContext, project: &ProjectMap) -> Vec<PathBuf> {
    if project.features.legacy_mode {
        project
            .paths
            .code
            .as_ref()
            .map(|code| {
                code.src
                    .iter()
                    .map(|path| context.repo_path(path))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    } else {
        vec![context.generated_code_path(project)]
    }
}

fn scan_source_root(
    root: &Path,
    repo_root: &Path,
    files_scanned: &mut usize,
    symbols: &mut Vec<Symbol>,
) -> Result<()> {
    if should_ignore_dir(root) {
        tracing::debug!(path = %root.display(), "Skipping ignored index root");
        return Ok(());
    }
    if !root.is_dir() {
        tracing::debug!(path = %root.display(), "Skipping missing index root");
        return Ok(());
    }

    for entry in std::fs::read_dir(root).with_context(|| format!("read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if should_ignore_dir(&path) {
                tracing::debug!(path = %path.display(), "Skipping ignored index directory");
                continue;
            }
            scan_source_root(&path, repo_root, files_scanned, symbols)?;
            continue;
        }

        if !path.is_file() {
            continue;
        }

        let relative = path.strip_prefix(repo_root).unwrap_or(&path);
        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => {
                tracing::debug!(path = %path.display(), "Skipping non-UTF8 source file");
                continue;
            }
        };
        let mut file_symbols = extract_symbols_from_source(relative, &content)?;
        if file_symbols.is_empty() {
            continue;
        }
        *files_scanned += 1;
        symbols.append(&mut file_symbols);
    }

    Ok(())
}

fn should_ignore_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| IGNORED_DIRS.contains(&name))
}

fn write_index_atomic(path: &Path, index: &Index) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    index.save(&tmp)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
