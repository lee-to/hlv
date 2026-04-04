use std::collections::HashSet;
use std::path::Path;

use crate::check::Diagnostic;
use crate::model::llm_map::{LlmMap, MapEntryKind};
use crate::model::project::LlmPaths;

/// Validate llm/map.yaml:
/// 1. Forward: every entry in map exists on disk (MAP-010)
/// 2. Reverse: every file in tracked dirs exists in map (MAP-020)
/// 3. Path isolation: llm-layer entries must be inside configured paths (MAP-030)
pub fn check_llm_map(root: &Path, map_rel: &str, llm_paths: &LlmPaths) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let full_path = root.join(map_rel);

    if !full_path.exists() {
        diags.push(
            Diagnostic::error("MAP-001", format!("Map file not found: {}", map_rel))
                .with_file(map_rel),
        );
        return diags;
    }

    let map = match LlmMap::load(&full_path) {
        Ok(m) => m,
        Err(e) => {
            diags.push(
                Diagnostic::error("MAP-002", format!("Cannot parse {}: {}", map_rel, e))
                    .with_file(map_rel),
            );
            return diags;
        }
    };

    if map.entries.is_empty() {
        diags.push(Diagnostic::info(
            "MAP-003",
            "Map has no entries (add entries as you create files)".to_string(),
        ));
        return diags;
    }

    // --- Forward check: every map entry exists on disk ---
    let mut found = 0usize;

    for entry in &map.entries {
        let p = root.join(&entry.path);
        let exists = match entry.kind {
            MapEntryKind::File => p.is_file(),
            MapEntryKind::Dir => p.is_dir(),
        };
        if exists {
            found += 1;
        } else {
            diags.push(
                Diagnostic::error(
                    "MAP-010",
                    format!(
                        "{} not found: {} (expected {})",
                        entry.layer, entry.path, entry.kind
                    ),
                )
                .with_file(map_rel),
            );
        }
    }

    let total = map.entries.len();
    diags.push(Diagnostic::info(
        "MAP-100",
        format!("Forward: {}/{} entries exist on disk", found, total),
    ));

    // --- Path isolation: llm-layer entries must be inside configured paths (MAP-030) ---
    let llm_src = normalize(&llm_paths.src);
    let llm_tests = llm_paths.tests.as_deref().map(normalize);

    for entry in &map.entries {
        if entry.layer != "llm" {
            continue;
        }
        let norm_path = normalize(&entry.path);
        let inside_src = norm_path.starts_with(&llm_src);
        let inside_tests = llm_tests.as_ref().is_some_and(|t| norm_path.starts_with(t));

        if !inside_src && !inside_tests {
            let expected = match &llm_tests {
                Some(t) => format!("{}, {}", llm_paths.src, t),
                None => llm_paths.src.clone(),
            };
            diags.push(
                Diagnostic::error(
                    "MAP-030",
                    format!(
                        "'{}' is layer:llm but outside configured paths (expected: {})",
                        entry.path, expected,
                    ),
                )
                .with_file(map_rel),
            );
        }
    }

    // --- Reverse check: every real file/dir in tracked dirs is in map ---
    let map_rel_normalized = map_rel.trim_end_matches('/');

    let known_paths: HashSet<String> = map
        .entries
        .iter()
        .map(|e| normalize(e.path.as_str()))
        .collect();

    let ignore_patterns = build_ignore_patterns(&map.ignore);

    // Scan every dir entry from the map
    let scan_dirs: Vec<&str> = map
        .entries
        .iter()
        .filter(|e| e.kind == MapEntryKind::Dir)
        .map(|e| e.path.as_str())
        .collect();

    let mut unlisted: Vec<String> = Vec::new();

    for dir_path in &scan_dirs {
        let full_dir = root.join(dir_path);
        if !full_dir.is_dir() {
            continue; // already reported by forward check
        }
        scan_unlisted(
            &full_dir,
            root,
            &known_paths,
            map_rel_normalized,
            &ignore_patterns,
            &mut unlisted,
        );
    }

    for path in &unlisted {
        diags.push(
            Diagnostic::warning("MAP-020", format!("File on disk not in map: {}", path))
                .with_file(map_rel),
        );
    }

    let unlisted_count = unlisted.len();
    if unlisted_count == 0 {
        diags.push(Diagnostic::info(
            "MAP-101",
            "Reverse: all files in tracked dirs are in map".to_string(),
        ));
    } else {
        diags.push(Diagnostic::info(
            "MAP-101",
            format!("Reverse: {} file(s) on disk not in map", unlisted_count),
        ));
    }

    diags
}

/// Build compiled glob patterns from ignore list.
fn build_ignore_patterns(ignore: &[String]) -> Vec<glob::Pattern> {
    ignore
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect()
}

/// Check if a relative path matches any ignore pattern.
/// Matches against the full relative path and also against each path component
/// (so `__pycache__` matches `llm/src/__pycache__/`).
fn is_ignored(rel_path: &str, patterns: &[glob::Pattern]) -> bool {
    let path = Path::new(rel_path);
    for pat in patterns {
        // Match full relative path
        if pat.matches(rel_path) {
            return true;
        }
        // Match any individual path component (e.g. __pycache__, node_modules)
        for component in path.components() {
            if let std::path::Component::Normal(c) = component {
                if pat.matches(&c.to_string_lossy()) {
                    return true;
                }
            }
        }
    }
    false
}

/// Recursively scan `dir` for files/subdirs not present in `known`.
fn scan_unlisted(
    dir: &Path,
    root: &Path,
    known: &HashSet<String>,
    map_rel: &str,
    ignore: &[glob::Pattern],
    unlisted: &mut Vec<String>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden files/dirs (.gitkeep, .DS_Store, etc.)
        if name_str.starts_with('.') {
            continue;
        }

        let path = entry.path();
        let rel = match path.strip_prefix(root) {
            Ok(r) => r.to_string_lossy().to_string(),
            Err(_) => continue,
        };
        let rel_norm = normalize(&rel);

        // Skip the map file itself
        if rel_norm == map_rel {
            continue;
        }

        // Skip ignored patterns
        if is_ignored(&rel_norm, ignore) {
            continue;
        }

        if path.is_dir() {
            // Check dir itself (with trailing slash — try both forms)
            if !known.contains(&rel_norm) && !known.contains(&format!("{}/", rel_norm)) {
                unlisted.push(format!("{}/", rel_norm));
            }
            // Recurse into subdir
            scan_unlisted(&path, root, known, map_rel, ignore, unlisted);
        } else if path.is_file() && !known.contains(&rel_norm) {
            unlisted.push(rel_norm);
        }
    }
}

fn normalize(path: &str) -> String {
    path.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn default_llm_paths() -> LlmPaths {
        LlmPaths {
            src: "llm/src/".to_string(),
            tests: Some("llm/tests/".to_string()),
            map: Some("llm/map.yaml".to_string()),
        }
    }

    /// Convenience: paths where src root IS the map root (flat layout).
    fn flat_llm_paths() -> LlmPaths {
        LlmPaths {
            src: "src/".to_string(),
            tests: Some("tests/".to_string()),
            map: Some("map.yaml".to_string()),
        }
    }

    #[test]
    fn all_entries_exist() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("human/constraints")).unwrap();
        fs::write(root.join("glossary.yaml"), "types: {}").unwrap();

        fs::write(
            root.join("map.yaml"),
            r#"
schema_version: 1
entries:
  - path: glossary.yaml
    kind: file
    layer: human
    description: "Glossary"
  - path: human/constraints
    kind: dir
    layer: human
    description: "Constraints"
"#,
        )
        .unwrap();

        let diags = check_llm_map(root, "map.yaml", &flat_llm_paths());
        assert!(
            !diags.iter().any(|d| d.code == "MAP-010"),
            "should have no MAP-010 errors"
        );
    }

    #[test]
    fn missing_entry_reports_error() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(
            root.join("map.yaml"),
            r#"
schema_version: 1
entries:
  - path: does/not/exist.yaml
    kind: file
    layer: human
    description: "Missing file"
"#,
        )
        .unwrap();

        let diags = check_llm_map(root, "map.yaml", &flat_llm_paths());
        assert!(diags.iter().any(|d| d.code == "MAP-010"));
    }

    #[test]
    fn missing_map_file_reports_error() {
        let dir = tempfile::tempdir().unwrap();
        let diags = check_llm_map(dir.path(), "nonexistent.yaml", &flat_llm_paths());
        assert!(diags.iter().any(|d| d.code == "MAP-001"));
    }

    #[test]
    fn kind_mismatch_reports_error() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Create a file, but map expects a dir
        fs::write(root.join("something"), "data").unwrap();
        fs::write(
            root.join("map.yaml"),
            r#"
schema_version: 1
entries:
  - path: something
    kind: dir
    layer: llm
    description: "Should be a dir"
"#,
        )
        .unwrap();

        let diags = check_llm_map(root, "map.yaml", &flat_llm_paths());
        assert!(diags.iter().any(|d| d.code == "MAP-010"));
    }

    #[test]
    fn reverse_detects_unlisted_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Create dir with two files
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/known.rs"), "fn main() {}").unwrap();
        fs::write(root.join("src/forgotten.rs"), "fn oops() {}").unwrap();

        // Map only lists one file + the dir
        fs::write(
            root.join("map.yaml"),
            r#"
schema_version: 1
entries:
  - path: src/
    kind: dir
    layer: llm
    description: "Source code"
  - path: src/known.rs
    kind: file
    layer: llm
    description: "Known file"
"#,
        )
        .unwrap();

        let diags = check_llm_map(root, "map.yaml", &flat_llm_paths());
        let unlisted: Vec<_> = diags.iter().filter(|d| d.code == "MAP-020").collect();
        assert_eq!(unlisted.len(), 1);
        assert!(unlisted[0].message.contains("forgotten.rs"));
    }

    #[test]
    fn reverse_detects_unlisted_subdir() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("src/features/new_feature")).unwrap();
        fs::write(root.join("src/features/new_feature/mod.rs"), "").unwrap();

        // Map only lists src/ — not the subdir or file
        fs::write(
            root.join("map.yaml"),
            r#"
schema_version: 1
entries:
  - path: src/
    kind: dir
    layer: llm
    description: "Source"
"#,
        )
        .unwrap();

        let diags = check_llm_map(root, "map.yaml", &flat_llm_paths());
        let unlisted: Vec<_> = diags.iter().filter(|d| d.code == "MAP-020").collect();
        assert!(unlisted.len() >= 2, "should detect subdir and file");
    }

    #[test]
    fn reverse_ignores_hidden_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/.gitkeep"), "").unwrap();
        fs::write(root.join("src/.DS_Store"), "").unwrap();

        fs::write(
            root.join("map.yaml"),
            r#"
schema_version: 1
entries:
  - path: src/
    kind: dir
    layer: llm
    description: "Source"
"#,
        )
        .unwrap();

        let diags = check_llm_map(root, "map.yaml", &flat_llm_paths());
        assert!(
            !diags.iter().any(|d| d.code == "MAP-020"),
            "hidden files should not trigger MAP-020"
        );
    }

    #[test]
    fn reverse_no_warnings_when_complete() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

        fs::write(
            root.join("map.yaml"),
            r#"
schema_version: 1
entries:
  - path: src/
    kind: dir
    layer: llm
    description: "Source"
  - path: src/main.rs
    kind: file
    layer: llm
    description: "Entry point"
"#,
        )
        .unwrap();

        let diags = check_llm_map(root, "map.yaml", &flat_llm_paths());
        assert!(
            !diags.iter().any(|d| d.code == "MAP-020"),
            "all files listed — no MAP-020"
        );
        assert!(diags
            .iter()
            .any(|d| d.code == "MAP-101" && d.message.contains("all files")));
    }

    #[test]
    fn reverse_ignores_patterns() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Create src with __pycache__ and a .pyc file
        fs::create_dir_all(root.join("src/__pycache__")).unwrap();
        fs::write(root.join("src/__pycache__/mod.cpython-313.pyc"), "").unwrap();
        fs::write(root.join("src/main.py"), "print('hi')").unwrap();
        fs::write(root.join("src/helper.pyc"), "bytecode").unwrap();

        fs::write(
            root.join("map.yaml"),
            r#"
schema_version: 1
ignore:
  - __pycache__
  - "*.pyc"
entries:
  - path: src/
    kind: dir
    layer: llm
    description: "Source"
  - path: src/main.py
    kind: file
    layer: llm
    description: "Entry point"
"#,
        )
        .unwrap();

        let diags = check_llm_map(root, "map.yaml", &flat_llm_paths());
        let unlisted: Vec<_> = diags.iter().filter(|d| d.code == "MAP-020").collect();
        assert!(
            unlisted.is_empty(),
            "ignored patterns should not trigger MAP-020, got: {:?}",
            unlisted.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn reverse_ignores_node_modules() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("src/node_modules/lodash")).unwrap();
        fs::write(root.join("src/node_modules/lodash/index.js"), "").unwrap();
        fs::write(root.join("src/index.ts"), "").unwrap();

        fs::write(
            root.join("map.yaml"),
            r#"
schema_version: 1
ignore:
  - node_modules
entries:
  - path: src/
    kind: dir
    layer: llm
    description: "Source"
  - path: src/index.ts
    kind: file
    layer: llm
    description: "Entry"
"#,
        )
        .unwrap();

        let diags = check_llm_map(root, "map.yaml", &flat_llm_paths());
        let unlisted: Vec<_> = diags.iter().filter(|d| d.code == "MAP-020").collect();
        assert!(
            unlisted.is_empty(),
            "node_modules should be ignored, got: {:?}",
            unlisted.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn reverse_skips_map_file_itself() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("llm")).unwrap();
        fs::write(
            root.join("llm/map.yaml"),
            r#"
schema_version: 1
entries:
  - path: llm/
    kind: dir
    layer: llm
    description: "LLM dir"
"#,
        )
        .unwrap();

        let diags = check_llm_map(root, "llm/map.yaml", &default_llm_paths());
        assert!(
            !diags.iter().any(|d| d.code == "MAP-020"),
            "map.yaml itself should not trigger MAP-020"
        );
    }

    #[test]
    fn path_isolation_detects_llm_outside_configured() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // llm entry at apps/backend/src/ but config says llm/src/
        fs::create_dir_all(root.join("apps/backend/src")).unwrap();
        fs::write(root.join("apps/backend/src/handler.ts"), "").unwrap();

        fs::write(
            root.join("map.yaml"),
            r#"
schema_version: 1
entries:
  - path: apps/backend/src/handler.ts
    kind: file
    layer: llm
    description: "Handler"
"#,
        )
        .unwrap();

        let paths = LlmPaths {
            src: "llm/src/".to_string(),
            tests: Some("llm/tests/".to_string()),
            map: Some("map.yaml".to_string()),
        };

        let diags = check_llm_map(root, "map.yaml", &paths);
        let violations: Vec<_> = diags.iter().filter(|d| d.code == "MAP-030").collect();
        assert_eq!(violations.len(), 1);
        assert!(violations[0]
            .message
            .contains("apps/backend/src/handler.ts"));
        assert!(violations[0].message.contains("llm/src/"));
    }

    #[test]
    fn path_isolation_allows_llm_inside_configured() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("llm/src/features")).unwrap();
        fs::write(root.join("llm/src/features/order.rs"), "").unwrap();
        fs::create_dir_all(root.join("llm/tests")).unwrap();
        fs::write(root.join("llm/tests/integration.rs"), "").unwrap();

        fs::write(
            root.join("map.yaml"),
            r#"
schema_version: 1
entries:
  - path: llm/src/features/order.rs
    kind: file
    layer: llm
    description: "Order handler"
  - path: llm/tests/integration.rs
    kind: file
    layer: llm
    description: "Integration tests"
"#,
        )
        .unwrap();

        let diags = check_llm_map(root, "map.yaml", &default_llm_paths());
        assert!(
            !diags.iter().any(|d| d.code == "MAP-030"),
            "files inside configured paths should not trigger MAP-030"
        );
    }

    #[test]
    fn path_isolation_ignores_non_llm_layers() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("human/contracts")).unwrap();
        fs::write(root.join("human/contracts/order.md"), "").unwrap();

        fs::write(
            root.join("map.yaml"),
            r#"
schema_version: 1
entries:
  - path: human/contracts/order.md
    kind: file
    layer: human
    description: "Order contract"
"#,
        )
        .unwrap();

        let diags = check_llm_map(root, "map.yaml", &default_llm_paths());
        assert!(
            !diags.iter().any(|d| d.code == "MAP-030"),
            "human layer should not trigger MAP-030"
        );
    }

    #[test]
    fn path_isolation_works_with_custom_paths() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Custom paths: apps/backend/src/ and apps/backend/test/
        fs::create_dir_all(root.join("apps/backend/src")).unwrap();
        fs::write(root.join("apps/backend/src/handler.ts"), "").unwrap();

        fs::write(
            root.join("map.yaml"),
            r#"
schema_version: 1
entries:
  - path: apps/backend/src/handler.ts
    kind: file
    layer: llm
    description: "Handler"
"#,
        )
        .unwrap();

        let paths = LlmPaths {
            src: "apps/backend/src/".to_string(),
            tests: Some("apps/backend/test/".to_string()),
            map: Some("map.yaml".to_string()),
        };

        let diags = check_llm_map(root, "map.yaml", &paths);
        assert!(
            !diags.iter().any(|d| d.code == "MAP-030"),
            "file inside custom configured path should not trigger MAP-030"
        );
    }
}
