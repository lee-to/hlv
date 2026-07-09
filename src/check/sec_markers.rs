use std::collections::HashMap;
use std::path::Path;

use regex::Regex;

use crate::check::Diagnostic;

/// Valid @hlv:sec categories.
const VALID_CATEGORIES: &[&str] = &[
    "INPUT_VALIDATION",
    "DESERIALIZATION",
    "AUTH_BOUNDARY",
    "SECRET_HANDLING",
    "FILE_ACCESS",
    "CRYPTO",
    "PRIVILEGE_ESCALATION",
    "NETWORK",
];

const IGNORED_MARKER_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "vendor",
    "__pycache__",
    ".venv",
    "dist",
    "build",
    ".pytest_cache",
    ".ruff_cache",
    ".mypy_cache",
];

/// A single @hlv:sec marker found in source code.
#[derive(Debug)]
#[allow(dead_code)]
struct SecMarker {
    category: String,
    file: String,
    line: usize,
}

/// Check for @hlv:sec security attention markers in source code.
/// Returns SEC-010 Info diagnostic with summary table of markers by category and file.
pub fn check_sec_markers(root: &Path, src_path: &str, markers_enabled: bool) -> Vec<Diagnostic> {
    check_sec_markers_with_scope(root, src_path, markers_enabled, None)
}

/// Check security attention markers, optionally limiting scans to an explicit
/// set of repository-relative changed files.
pub fn check_sec_markers_with_scope(
    root: &Path,
    src_path: &str,
    markers_enabled: bool,
    changed_files: Option<&[String]>,
) -> Vec<Diagnostic> {
    if !markers_enabled {
        tracing::debug!("Skipping security markers check — security_markers disabled");
        return Vec::new();
    }

    if changed_files.is_some_and(|files| files.is_empty()) {
        tracing::debug!("Skipping security markers check — legacy mode has no changed files");
        return Vec::new();
    }

    let re = Regex::new(r"@hlv:sec\s+\[(\w+)\]").expect("valid regex");
    let mut markers: Vec<SecMarker> = Vec::new();
    let mut invalid_categories: Vec<(String, String, usize)> = Vec::new();

    if let Some(files) = changed_files {
        tracing::debug!(
            file_count = files.len(),
            "Scanning legacy changed files for @hlv:sec markers"
        );
        for rel in files {
            scan_sec_marker_file(
                &root.join(rel),
                root,
                &re,
                &mut markers,
                &mut invalid_categories,
            );
        }
    } else {
        let full_src = root.join(src_path);
        if !full_src.exists() {
            tracing::debug!("Source path does not exist: {}", full_src.display());
            return Vec::new();
        }
        scan_sec_markers(&full_src, root, &re, &mut markers, &mut invalid_categories);
    }

    let mut diags = Vec::new();

    // Report invalid categories
    for (cat, file, line) in &invalid_categories {
        diags.push(
            Diagnostic::warning(
                "SEC-011",
                format!(
                    "Unknown @hlv:sec category '{}' at line {} (valid: {})",
                    cat,
                    line,
                    VALID_CATEGORIES.join(", ")
                ),
            )
            .with_file(file),
        );
    }

    if markers.is_empty() && invalid_categories.is_empty() {
        return diags;
    }

    // Build summary: count by category
    let mut by_category: HashMap<&str, usize> = HashMap::new();
    let mut by_file: HashMap<String, usize> = HashMap::new();
    for m in &markers {
        *by_category.entry(&m.category).or_insert(0) += 1;
        *by_file.entry(m.file.clone()).or_insert(0) += 1;
    }

    let total = markers.len();

    // Build summary message
    let mut summary_parts: Vec<String> = Vec::new();
    let mut cats: Vec<&&str> = by_category.keys().collect();
    cats.sort();
    for cat in cats {
        summary_parts.push(format!("{}={}", cat, by_category[cat]));
    }
    let cat_summary = summary_parts.join(", ");

    let file_count = by_file.len();

    let message = format!(
        "Security markers: {} total across {} file(s) [{}]",
        total, file_count, cat_summary
    );

    diags.push(Diagnostic::info("SEC-010", message));

    diags
}

/// Recursively scan directory for @hlv:sec markers.
fn scan_sec_markers(
    dir: &Path,
    root: &Path,
    re: &Regex,
    markers: &mut Vec<SecMarker>,
    invalid: &mut Vec<(String, String, usize)>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip common build/dependency directories
        if path.is_dir() {
            if IGNORED_MARKER_DIRS.contains(&name.as_str()) {
                continue;
            }
            scan_sec_markers(&path, root, re, markers, invalid);
            continue;
        }

        if !path.is_file() {
            continue;
        }

        scan_sec_marker_file(&path, root, re, markers, invalid);
    }
}

fn scan_sec_marker_file(
    path: &Path,
    root: &Path,
    re: &Regex,
    markers: &mut Vec<SecMarker>,
    invalid: &mut Vec<(String, String, usize)>,
) {
    if !path.is_file() {
        return;
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let rel_path = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    for (line_num, line) in content.lines().enumerate() {
        for cap in re.captures_iter(line) {
            let category = cap[1].to_string();
            if VALID_CATEGORIES.contains(&category.as_str()) {
                markers.push(SecMarker {
                    category,
                    file: rel_path.clone(),
                    line: line_num + 1,
                });
            } else {
                invalid.push((category, rel_path.clone(), line_num + 1));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn disabled_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(
            dir.path().join("src/main.rs"),
            "// @hlv:sec [INPUT_VALIDATION] — check user input",
        )
        .unwrap();

        let diags = check_sec_markers(dir.path(), "src", false);
        assert!(diags.is_empty());
    }

    #[test]
    fn no_markers_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();

        let diags = check_sec_markers(dir.path(), "src", true);
        assert!(diags.is_empty());
    }

    #[test]
    fn finds_valid_markers() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(
            dir.path().join("src/handler.rs"),
            r#"
// @hlv:sec [INPUT_VALIDATION] — validate user email
fn validate_email() {}

// @hlv:sec [AUTH_BOUNDARY] — check session token
fn check_auth() {}

// @hlv:sec [INPUT_VALIDATION] — validate quantity
fn validate_qty() {}
"#,
        )
        .unwrap();

        let diags = check_sec_markers(dir.path(), "src", true);
        let sec010 = diags.iter().find(|d| d.code == "SEC-010");
        assert!(sec010.is_some(), "should have SEC-010 summary");
        let msg = &sec010.unwrap().message;
        assert!(msg.contains("3 total"), "msg: {}", msg);
        assert!(msg.contains("1 file(s)"), "msg: {}", msg);
        assert!(msg.contains("AUTH_BOUNDARY=1"), "msg: {}", msg);
        assert!(msg.contains("INPUT_VALIDATION=2"), "msg: {}", msg);
    }

    #[test]
    fn invalid_category_warns() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(
            dir.path().join("src/main.rs"),
            "// @hlv:sec [BOGUS_CATEGORY] — something",
        )
        .unwrap();

        let diags = check_sec_markers(dir.path(), "src", true);
        let warnings: Vec<_> = diags.iter().filter(|d| d.code == "SEC-011").collect();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("BOGUS_CATEGORY"));
    }

    #[test]
    fn skips_git_and_target_dirs() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/.git")).unwrap();
        fs::create_dir_all(dir.path().join("src/target")).unwrap();
        fs::create_dir_all(dir.path().join("src/real")).unwrap();

        fs::write(
            dir.path().join("src/.git/file.rs"),
            "// @hlv:sec [CRYPTO] — hidden",
        )
        .unwrap();
        fs::write(
            dir.path().join("src/target/file.rs"),
            "// @hlv:sec [CRYPTO] — hidden",
        )
        .unwrap();
        fs::write(
            dir.path().join("src/real/file.rs"),
            "// @hlv:sec [CRYPTO] — real marker",
        )
        .unwrap();

        let diags = check_sec_markers(dir.path(), "src", true);
        let sec010 = diags.iter().find(|d| d.code == "SEC-010");
        assert!(sec010.is_some());
        assert!(sec010.unwrap().message.contains("1 total"));
    }

    #[test]
    fn all_eight_categories_recognized() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();

        let mut content = String::new();
        for cat in VALID_CATEGORIES {
            content.push_str(&format!("// @hlv:sec [{}] — test\n", cat));
        }
        fs::write(dir.path().join("src/all.rs"), &content).unwrap();

        let diags = check_sec_markers(dir.path(), "src", true);
        assert!(
            !diags.iter().any(|d| d.code == "SEC-011"),
            "no invalid category warnings"
        );
        let sec010 = diags.iter().find(|d| d.code == "SEC-010").unwrap();
        assert!(sec010.message.contains("8 total"));
    }

    #[test]
    fn nonexistent_src_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let diags = check_sec_markers(dir.path(), "nonexistent", true);
        assert!(diags.is_empty());
    }
}
