use std::collections::HashSet;
use std::path::Path;

use crate::check::Diagnostic;
use crate::model::contract_yaml::ContractYaml;
use crate::model::project::{ConstraintEntry, ContractEntry};

/// Expected marker that must appear in generated code.
#[derive(Debug)]
struct ExpectedMarker {
    id: String,
    kind: &'static str,
    source: String,
}

/// Check that every contract error, invariant, and constraint rule
/// has a corresponding `@hlv <ID>` marker in generated code.
pub fn check_code_trace(
    root: &Path,
    contracts: &[ContractEntry],
    constraints: &[ConstraintEntry],
    src_path: &str,
    tests_path: Option<&str>,
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    // 1. Collect expected markers from contracts
    let mut expected: Vec<ExpectedMarker> = Vec::new();

    for entry in contracts {
        if let Some(yaml_path) = &entry.yaml_path {
            let full = root.join(yaml_path);
            match ContractYaml::load(&full) {
                Ok(contract) => {
                    for err in &contract.errors {
                        expected.push(ExpectedMarker {
                            id: err.code.clone(),
                            kind: "error",
                            source: entry.id.clone(),
                        });
                    }
                    for inv in &contract.invariants {
                        expected.push(ExpectedMarker {
                            id: inv.id.clone(),
                            kind: "invariant",
                            source: entry.id.clone(),
                        });
                    }
                }
                Err(_) => continue,
            }
        }
    }

    // 2. Collect expected markers from constraints (rules[].id)
    for constraint in constraints {
        let full = root.join(&constraint.path);
        let content = match std::fs::read_to_string(&full) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let value: serde_yaml::Value = match serde_yaml::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(rules) = value.get("rules").and_then(|r| r.as_sequence()) {
            for rule in rules {
                if let Some(id) = rule.get("id").and_then(|i| i.as_str()) {
                    // Rules with check_command are verified programmatically, not via @hlv markers
                    if rule.get("check_command").is_some() {
                        continue;
                    }
                    expected.push(ExpectedMarker {
                        id: id.to_string(),
                        kind: "constraint",
                        source: constraint.id.clone(),
                    });
                }
            }
        }
    }

    if expected.is_empty() {
        return diags;
    }

    // 3. Scan source code for @hlv markers
    let mut found_markers: HashSet<String> = HashSet::new();

    let mut scan_dirs: Vec<&str> = vec![src_path];
    if let Some(t) = tests_path {
        scan_dirs.push(t);
    }

    for dir in scan_dirs {
        let full = root.join(dir);
        if full.exists() {
            scan_for_markers(&full, &mut found_markers);
        }
    }

    // 4. Report missing markers
    for marker in &expected {
        if !found_markers.contains(&marker.id) {
            diags.push(Diagnostic::warning(
                "CTR-010",
                format!(
                    "{} '{}' from {} has no @hlv marker in code",
                    marker.kind, marker.id, marker.source
                ),
            ));
        }
    }

    // 5. Summary
    let covered = expected
        .iter()
        .filter(|m| found_markers.contains(&m.id))
        .count();
    let total = expected.len();
    diags.push(Diagnostic::info(
        "CTR-001",
        format!("Code traceability: {}/{} markers covered", covered, total),
    ));

    diags
}

fn scan_for_markers(dir: &Path, markers: &mut HashSet<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_for_markers(&path, markers);
        } else if path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                for line in content.lines() {
                    if let Some(pos) = line.find("@hlv") {
                        let rest = line[pos + 4..].trim_start();
                        if let Some(id) = rest.split_whitespace().next() {
                            markers.insert(id.to_string());
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn scan_finds_hlv_markers() {
        let dir = tempfile::tempdir().unwrap();
        let test_file = dir.path().join("test_order.rs");
        fs::write(
            &test_file,
            r#"
// @hlv OUT_OF_STOCK
#[test]
fn test_out_of_stock() {}

// @hlv atomicity
#[test]
fn test_atomicity() {}
"#,
        )
        .unwrap();

        let mut markers = HashSet::new();
        scan_for_markers(dir.path(), &mut markers);
        assert!(markers.contains("OUT_OF_STOCK"));
        assert!(markers.contains("atomicity"));
        assert_eq!(markers.len(), 2);
    }

    #[test]
    fn scan_handles_different_comment_styles() {
        let dir = tempfile::tempdir().unwrap();
        let test_file = dir.path().join("test.py");
        fs::write(
            &test_file,
            r#"
# @hlv INVALID_QUANTITY
def test_invalid_quantity():
    pass

# @hlv prepared_statements_only
def test_sql():
    pass
"#,
        )
        .unwrap();

        let mut markers = HashSet::new();
        scan_for_markers(dir.path(), &mut markers);
        assert!(markers.contains("INVALID_QUANTITY"));
        assert!(markers.contains("prepared_statements_only"));
    }

    #[test]
    fn check_code_trace_empty_contracts() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("llm/src")).unwrap();

        let diags = check_code_trace(root, &[], &[], "llm/src", None);
        assert!(diags.is_empty(), "no contracts = no expected markers");
    }

    #[test]
    fn check_code_trace_contract_with_markers() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Create contract YAML with errors and invariants
        fs::create_dir_all(root.join("contracts")).unwrap();
        fs::write(
            root.join("contracts/order.create.yaml"),
            r#"id: order.create
version: 1.0.0
errors:
  - code: OUT_OF_STOCK
    http_status: 409
  - code: INVALID_QUANTITY
    http_status: 400
invariants:
  - id: atomicity
inputs_schema:
  type: object
outputs_schema:
  type: object
"#,
        )
        .unwrap();

        // Create source with all markers
        fs::create_dir_all(root.join("llm/src")).unwrap();
        fs::write(
            root.join("llm/src/handler.rs"),
            r#"
// @hlv OUT_OF_STOCK
fn handle_out_of_stock() {}
// @hlv INVALID_QUANTITY
fn handle_invalid_qty() {}
// @hlv atomicity
fn test_atomicity() {}
"#,
        )
        .unwrap();

        let contracts = vec![ContractEntry {
            id: "order.create".to_string(),
            version: "1.0.0".to_string(),
            path: "contracts/order.create.md".to_string(),
            yaml_path: Some("contracts/order.create.yaml".to_string()),
            owner: None,
            status: crate::model::project::ContractStatus::Draft,
            test_spec: None,
            depends_on: vec![],
            artifacts: vec![],
        }];

        let diags = check_code_trace(root, &contracts, &[], "llm/src", None);

        // Should have CTR-001 summary but no CTR-010 warnings
        assert!(
            !diags.iter().any(|d| d.code == "CTR-010"),
            "all markers present, no CTR-010: {:?}",
            diags
        );
        let summary = diags.iter().find(|d| d.code == "CTR-001");
        assert!(summary.is_some(), "should have CTR-001 summary");
        assert!(summary.unwrap().message.contains("3/3"));
    }

    #[test]
    fn check_code_trace_missing_markers() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("contracts")).unwrap();
        fs::write(
            root.join("contracts/order.create.yaml"),
            r#"id: order.create
version: 1.0.0
errors:
  - code: OUT_OF_STOCK
    http_status: 409
invariants:
  - id: atomicity
inputs_schema:
  type: object
outputs_schema:
  type: object
"#,
        )
        .unwrap();

        // Empty source — no markers
        fs::create_dir_all(root.join("llm/src")).unwrap();
        fs::write(root.join("llm/src/main.rs"), "fn main() {}").unwrap();

        let contracts = vec![ContractEntry {
            id: "order.create".to_string(),
            version: "1.0.0".to_string(),
            path: "contracts/order.create.md".to_string(),
            yaml_path: Some("contracts/order.create.yaml".to_string()),
            owner: None,
            status: crate::model::project::ContractStatus::Draft,
            test_spec: None,
            depends_on: vec![],
            artifacts: vec![],
        }];

        let diags = check_code_trace(root, &contracts, &[], "llm/src", None);
        let warnings: Vec<_> = diags.iter().filter(|d| d.code == "CTR-010").collect();
        assert_eq!(warnings.len(), 2, "2 missing markers: {:?}", warnings);
        assert!(diags.iter().any(|d| d.code == "CTR-001"));
    }

    #[test]
    fn check_code_trace_with_constraints() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Constraint with rules
        fs::create_dir_all(root.join("human/constraints")).unwrap();
        fs::write(
            root.join("human/constraints/security.yaml"),
            r#"id: security.global
version: 1.0.0
rules:
  - id: prepared_statements_only
    severity: critical
    statement: "Use prepared statements"
  - id: no_secrets_in_logs
    severity: critical
    statement: "No secrets in logs"
"#,
        )
        .unwrap();

        // Source with one marker
        fs::create_dir_all(root.join("llm/src")).unwrap();
        fs::write(
            root.join("llm/src/db.rs"),
            "// @hlv prepared_statements_only\nfn query() {}",
        )
        .unwrap();

        let constraints = vec![ConstraintEntry {
            id: "security.global".to_string(),
            path: "human/constraints/security.yaml".to_string(),
            applies_to: Some("all".to_string()),
        }];

        let diags = check_code_trace(root, &[], &constraints, "llm/src", None);
        // One marker found, one missing
        let warnings: Vec<_> = diags.iter().filter(|d| d.code == "CTR-010").collect();
        assert_eq!(
            warnings.len(),
            1,
            "1 missing constraint marker: {:?}",
            warnings
        );
        assert!(
            warnings[0].message.contains("no_secrets_in_logs"),
            "should mention the missing rule"
        );
    }

    #[test]
    fn check_code_trace_scans_tests_dir() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("contracts")).unwrap();
        fs::write(
            root.join("contracts/order.yaml"),
            r#"id: order
version: 1.0.0
errors:
  - code: ERR_1
    http_status: 400
    description: "err"
invariants: []
input:
  type: object
output:
  type: object
"#,
        )
        .unwrap();

        // Marker is in tests dir, not src
        fs::create_dir_all(root.join("llm/src")).unwrap();
        fs::create_dir_all(root.join("llm/tests")).unwrap();
        fs::write(root.join("llm/tests/test_order.rs"), "// @hlv ERR_1").unwrap();

        let contracts = vec![ContractEntry {
            id: "order".to_string(),
            version: "1.0.0".to_string(),
            path: "contracts/order.md".to_string(),
            yaml_path: Some("contracts/order.yaml".to_string()),
            owner: None,
            status: crate::model::project::ContractStatus::Draft,
            test_spec: None,
            depends_on: vec![],
            artifacts: vec![],
        }];

        let diags = check_code_trace(root, &contracts, &[], "llm/src", Some("llm/tests"));
        assert!(
            !diags.iter().any(|d| d.code == "CTR-010"),
            "marker found in tests dir should count: {:?}",
            diags
        );
    }

    #[test]
    fn check_code_trace_contract_without_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("llm/src")).unwrap();

        // Contract without yaml_path — should be skipped gracefully
        let contracts = vec![ContractEntry {
            id: "order".to_string(),
            version: "1.0.0".to_string(),
            path: "contracts/order.md".to_string(),
            yaml_path: None,
            owner: None,
            status: crate::model::project::ContractStatus::Draft,
            test_spec: None,
            depends_on: vec![],
            artifacts: vec![],
        }];

        let diags = check_code_trace(root, &contracts, &[], "llm/src", None);
        assert!(diags.is_empty(), "no yaml = no expected markers");
    }
}
