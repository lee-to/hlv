use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::check::Diagnostic;
use crate::model::index::{Index, Symbol};
use crate::model::llm_map::{LlmMap, MapEntryKind};
use crate::model::project::ProjectMap;

type SymbolGroupKey<'a> = (&'a str, Option<&'a str>, Option<&'a str>, &'a str, &'a str);

pub fn check_index(config_root: &Path, repo_root: &Path, project: &ProjectMap) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let index_path = config_root.join("index/signatures.yaml");
    if !index_path.exists() {
        diags.push(
            Diagnostic::warning(
                "IDX-010",
                "Signature index is missing; run hlv index build.",
            )
            .with_file("index/signatures.yaml"),
        );
        return diags;
    }

    let index = match Index::load(&index_path) {
        Ok(index) => index,
        Err(e) => {
            diags.push(
                Diagnostic::warning("IDX-010", format!("Cannot parse signature index: {e}"))
                    .with_file("index/signatures.yaml"),
            );
            return diags;
        }
    };

    check_stale_symbols(repo_root, &index.symbols, &mut diags);
    check_duplicate_symbols(&index.symbols, &mut diags);

    if let Some(map_path) = &project.paths.llm.map {
        let map = LlmMap::load(&config_root.join(map_path)).ok();
        if let Some(map) = map {
            check_map_index_refs(map_path, &map, &index.symbols, &mut diags);
        }
    }

    diags
}

fn check_stale_symbols(repo_root: &Path, symbols: &[Symbol], diags: &mut Vec<Diagnostic>) {
    for symbol in symbols {
        let file_path = repo_root.join(&symbol.file);
        let Ok(content) = std::fs::read_to_string(&file_path) else {
            diags.push(
                Diagnostic::warning(
                    "IDX-010",
                    format!(
                        "Indexed symbol '{}' points at missing file {}",
                        symbol.id, symbol.file
                    ),
                )
                .with_file("index/signatures.yaml"),
            );
            continue;
        };

        if !content.contains(&symbol.signature) {
            diags.push(
                Diagnostic::warning(
                    "IDX-010",
                    format!(
                        "Indexed symbol '{}' appears stale; signature not found in {}",
                        symbol.id, symbol.file
                    ),
                )
                .with_file("index/signatures.yaml"),
            );
        }
    }
}

fn check_map_index_refs(
    map_path: &str,
    map: &LlmMap,
    symbols: &[Symbol],
    diags: &mut Vec<Diagnostic>,
) {
    let symbol_ids: BTreeSet<&str> = symbols.iter().map(|symbol| symbol.id.as_str()).collect();

    for entry in &map.entries {
        // Directory entries are navigational; only file-level code entries are
        // expected to pin a concrete indexed symbol.
        if entry.layer == "code" && entry.kind == MapEntryKind::File && entry.index_ref.is_none() {
            diags.push(
                Diagnostic::warning(
                    "IDX-030",
                    format!("Code map entry '{}' has no index_ref", entry.path),
                )
                .with_file(map_path),
            );
        }

        if let Some(index_ref) = &entry.index_ref {
            if !symbol_ids.contains(index_ref.as_str()) {
                diags.push(
                    Diagnostic::warning(
                        "IDX-020",
                        format!(
                            "Map entry '{}' references missing symbol {}",
                            entry.path, index_ref
                        ),
                    )
                    .with_file(map_path),
                );
            }
        }
    }
}

fn check_duplicate_symbols(symbols: &[Symbol], diags: &mut Vec<Diagnostic>) {
    let mut groups: BTreeMap<SymbolGroupKey<'_>, Vec<&Symbol>> = BTreeMap::new();
    for symbol in symbols {
        groups
            .entry((
                symbol.language.as_str(),
                symbol.namespace.as_deref(),
                symbol.scope.as_deref(),
                symbol.name.as_str(),
                symbol.kind.as_str(),
            ))
            .or_default()
            .push(symbol);
    }

    for ((language, namespace, scope, name, kind), group) in groups {
        if group.len() <= 1 {
            continue;
        }
        let locations = group
            .iter()
            .map(|symbol| format!("{}:{}", symbol.file, symbol.line))
            .collect::<Vec<_>>()
            .join(", ");
        diags.push(
            Diagnostic::warning(
                "IDX-040",
                format!(
                    "Duplicate {language} {kind} symbol '{}' in namespace {:?}, scope {:?}: {}",
                    name, namespace, scope, locations
                ),
            )
            .with_file("index/signatures.yaml"),
        );
    }
}
