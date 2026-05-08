use std::process::Command;

use tempfile::TempDir;

fn hlv_bin() -> &'static str {
    env!("CARGO_BIN_EXE_hlv")
}

fn setup_legacy_project(root: &std::path::Path) {
    hlv::cmd::init::run_with_milestone(
        root.to_str().unwrap(),
        Some("legacy-project"),
        Some("platform"),
        Some("claude"),
        Some("init"),
        Some("minimal"),
    )
    .unwrap();
}

fn git(root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout: {}\nstderr: {}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn artifacts_audit_legacy_project_smoke() {
    let tmp = TempDir::new().unwrap();
    setup_legacy_project(tmp.path());

    let output = Command::new(hlv_bin())
        .args(["--root", tmp.path().to_str().unwrap(), "artifacts", "audit"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "audit should pass for legacy projects: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No artifact graph metadata found"),
        "expected legacy migration hint, got: {stdout}"
    );
}

#[test]
fn artifacts_audit_json_legacy_project_smoke() {
    let tmp = TempDir::new().unwrap();
    setup_legacy_project(tmp.path());

    let output = Command::new(hlv_bin())
        .args([
            "--root",
            tmp.path().to_str().unwrap(),
            "artifacts",
            "audit",
            "--json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["metadata_found"], false);
    assert_eq!(value["errors"], 0);
    assert_eq!(value["warnings"], 0);
    assert_eq!(value["exit_code"], 0);
    assert_eq!(value["diagnostics"].as_array().unwrap().len(), 0);
}

#[test]
fn artifacts_audit_ignores_legacy_id_status_frontmatter_smoke() {
    let tmp = TempDir::new().unwrap();
    setup_legacy_project(tmp.path());

    std::fs::write(
        tmp.path().join("human/artifacts/legacy-adr.md"),
        r#"---
id: adr-auth-session
status: accepted
---
# ADR
"#,
    )
    .unwrap();

    let output = Command::new(hlv_bin())
        .args(["--root", tmp.path().to_str().unwrap(), "artifacts", "audit"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "legacy id/status frontmatter should be ignored: stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("ART-001"),
        "legacy frontmatter should not emit ART-001: {stdout}"
    );
}

#[test]
fn artifacts_audit_errors_exit_nonzero_smoke() {
    let tmp = TempDir::new().unwrap();
    setup_legacy_project(tmp.path());

    std::fs::write(
        tmp.path().join("human/artifacts/spec-auth.md"),
        r#"---
id: spec-auth
type: spec
owners: [product]
depends_on: [missing-adr]
---
# Auth Spec
"#,
    )
    .unwrap();

    let output = Command::new(hlv_bin())
        .args(["--root", tmp.path().to_str().unwrap(), "artifacts", "audit"])
        .output()
        .unwrap();

    assert!(!output.status.success(), "audit should fail on ART errors");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ART-020"), "expected ART-020: {stdout}");
}

#[test]
fn artifacts_audit_json_errors_include_counts_smoke() {
    let tmp = TempDir::new().unwrap();
    setup_legacy_project(tmp.path());

    std::fs::write(
        tmp.path().join("human/artifacts/spec-auth.md"),
        r#"---
id: spec-auth
type: spec
owners: [product]
depends_on: [missing-adr]
---
# Auth Spec
"#,
    )
    .unwrap();

    let output = Command::new(hlv_bin())
        .args([
            "--root",
            tmp.path().to_str().unwrap(),
            "artifacts",
            "audit",
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "audit --json should fail on ART errors"
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["errors"], 1);
    assert_eq!(value["exit_code"], 1);
    assert_eq!(value["diagnostics"][0]["code"], "ART-020");
}

#[test]
fn artifacts_impact_fixture_smoke() {
    let output = Command::new(hlv_bin())
        .args([
            "--root",
            "tests/fixtures/example-project",
            "artifacts",
            "impact",
            "spec-checkout",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "impact should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Expected to review"));
    assert!(stdout.contains("code-checkout"));
    assert!(stdout.contains("tests-checkout"));
}

#[test]
fn artifacts_impact_json_reports_ownership_types_smoke() {
    let output = Command::new(hlv_bin())
        .args([
            "--root",
            "tests/fixtures/example-project",
            "artifacts",
            "impact",
            "spec-checkout",
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "impact --json should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let affected = value["affected"].as_array().unwrap();
    assert!(affected
        .iter()
        .any(|item| item["id"] == "code-checkout" && item["artifact_type"] == "code"));
    assert!(affected
        .iter()
        .any(|item| item["id"] == "tests-checkout" && item["artifact_type"] == "tests"));
}

#[test]
fn artifacts_graph_fixture_smoke() {
    let output = Command::new(hlv_bin())
        .args([
            "--root",
            "tests/fixtures/example-project",
            "artifacts",
            "graph",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "graph should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("artifact graph"));
    assert!(stdout.contains("spec-checkout (spec)"));
    assert!(stdout.contains("spec-checkout --affects--> code-checkout"));
    assert!(stdout.contains("code-checkout --implements--> spec-checkout"));
    assert!(stdout.contains("tests-checkout --verifies--> spec-checkout"));
}

#[test]
fn artifacts_graph_json_fixture_smoke() {
    let output = Command::new(hlv_bin())
        .args([
            "--root",
            "tests/fixtures/example-project",
            "artifacts",
            "graph",
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "graph --json should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|node| node["id"] == "spec-checkout"));
    assert!(value["edges"].as_array().unwrap().iter().any(|edge| {
        edge["source"] == "code-checkout"
            && edge["relation"] == "implements"
            && edge["target"] == "spec-checkout"
    }));
}

#[test]
fn artifacts_impact_changed_without_head_smoke() {
    let tmp = TempDir::new().unwrap();
    setup_legacy_project(tmp.path());

    let git = Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(
        git.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&git.stderr)
    );

    std::fs::write(
        tmp.path().join("human/artifacts/spec-auth.md"),
        r#"---
id: spec-auth
type: spec
owners: [product]
affects: [code-auth]
---
# Auth Spec
"#,
    )
    .unwrap();

    let project_path = tmp.path().join("project.yaml");
    let mut project = std::fs::read_to_string(&project_path).unwrap();
    project = project.replace(
        "  code_ownership: {}\n",
        r#"
  code_ownership:
    code-auth:
      paths: [llm/src/auth/**]
      owners: [platform]
      implements: [spec-auth]
"#,
    );
    std::fs::write(project_path, project).unwrap();

    let output = Command::new(hlv_bin())
        .args([
            "--root",
            tmp.path().to_str().unwrap(),
            "artifacts",
            "impact",
            "--changed",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "impact --changed should not require HEAD: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("spec-auth"),
        "expected changed artifact: {stdout}"
    );
    assert!(
        stdout.contains("code-auth"),
        "expected affected code node: {stdout}"
    );
}

#[test]
fn artifacts_impact_changed_matches_code_ownership_paths_smoke() {
    let tmp = TempDir::new().unwrap();
    setup_legacy_project(tmp.path());
    git(tmp.path(), &["init"]);
    git(tmp.path(), &["config", "user.email", "hlv@example.com"]);
    git(tmp.path(), &["config", "user.name", "HLV Test"]);

    std::fs::write(
        tmp.path().join("human/artifacts/spec-auth.md"),
        r#"---
id: spec-auth
type: spec
owners: [product]
affects: [code-auth]
---
# Auth Spec
"#,
    )
    .unwrap();
    std::fs::create_dir_all(tmp.path().join("llm/src/auth")).unwrap();
    std::fs::write(
        tmp.path().join("llm/src/auth/service.ts"),
        "// @hlv:artifact code-auth implements spec-auth\nexport class Auth {}\n",
    )
    .unwrap();

    let project_path = tmp.path().join("project.yaml");
    let mut project = std::fs::read_to_string(&project_path).unwrap();
    project = project.replace(
        "  code_ownership: {}\n",
        r#"
  code_ownership:
    code-auth:
      paths: [llm/src/auth/**]
      owners: [platform]
      implements: [spec-auth]
"#,
    );
    std::fs::write(project_path, project).unwrap();
    git(tmp.path(), &["add", "."]);
    git(tmp.path(), &["commit", "-m", "base"]);

    std::fs::write(
        tmp.path().join("llm/src/auth/service.ts"),
        "// @hlv:artifact code-auth implements spec-auth\nexport class Auth { login() {} }\n",
    )
    .unwrap();

    let output = Command::new(hlv_bin())
        .args([
            "--root",
            tmp.path().to_str().unwrap(),
            "artifacts",
            "impact",
            "--changed",
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "impact --changed should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["changed"], serde_json::json!(["code-auth"]));
}

#[test]
fn artifacts_impact_changed_base_uses_merge_base_smoke() {
    let tmp = TempDir::new().unwrap();
    setup_legacy_project(tmp.path());
    git(tmp.path(), &["init"]);
    git(tmp.path(), &["config", "user.email", "hlv@example.com"]);
    git(tmp.path(), &["config", "user.name", "HLV Test"]);

    std::fs::write(
        tmp.path().join("human/artifacts/spec-auth.md"),
        r#"---
id: spec-auth
type: spec
owners: [product]
affects: [code-auth]
---
# Auth Spec
"#,
    )
    .unwrap();
    std::fs::create_dir_all(tmp.path().join("llm/src/auth")).unwrap();
    std::fs::write(
        tmp.path().join("llm/src/auth/service.ts"),
        "// @hlv:artifact code-auth implements spec-auth\nexport class Auth {}\n",
    )
    .unwrap();
    let project_path = tmp.path().join("project.yaml");
    let mut project = std::fs::read_to_string(&project_path).unwrap();
    project = project.replace(
        "  code_ownership: {}\n",
        r#"
  code_ownership:
    code-auth:
      paths: [llm/src/auth/**]
      owners: [platform]
      implements: [spec-auth]
"#,
    );
    std::fs::write(project_path, project).unwrap();
    git(tmp.path(), &["add", "."]);
    git(tmp.path(), &["commit", "-m", "base"]);

    std::fs::write(
        tmp.path().join("llm/src/auth/service.ts"),
        "// @hlv:artifact code-auth implements spec-auth\nexport class Auth { login() {} }\n",
    )
    .unwrap();
    git(tmp.path(), &["add", "."]);
    git(tmp.path(), &["commit", "-m", "change auth"]);

    let output = Command::new(hlv_bin())
        .args([
            "--root",
            tmp.path().to_str().unwrap(),
            "artifacts",
            "impact",
            "--changed",
            "--base",
            "HEAD~1",
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "impact --changed --base should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["changed"], serde_json::json!(["code-auth"]));
}

#[test]
fn artifacts_impact_unknown_target_fails_smoke() {
    let output = Command::new(hlv_bin())
        .args([
            "--root",
            "tests/fixtures/example-project",
            "artifacts",
            "impact",
            "missing-artifact",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "unknown target should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unknown artifact id or path 'missing-artifact'"),
        "expected unknown-target error: {stderr}"
    );
}

#[test]
fn artifacts_sync_creates_ownership_stubs_smoke() {
    let tmp = TempDir::new().unwrap();
    setup_legacy_project(tmp.path());

    std::fs::write(
        tmp.path().join("human/artifacts/spec-auth.md"),
        r#"---
id: spec-auth
type: spec
owners: [product]
affects:
  - code-auth
  - tests-auth
---
# Auth Spec
"#,
    )
    .unwrap();

    let check = Command::new(hlv_bin())
        .args([
            "--root",
            tmp.path().to_str().unwrap(),
            "artifacts",
            "sync",
            "--check",
        ])
        .output()
        .unwrap();
    assert!(
        !check.status.success(),
        "sync --check should fail when stubs are missing"
    );

    let output = Command::new(hlv_bin())
        .args([
            "--root",
            tmp.path().to_str().unwrap(),
            "artifacts",
            "sync",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "sync should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["changed"], true);
    assert_eq!(value["missing"].as_array().unwrap().len(), 2);

    let project = hlv::model::project::ProjectMap::load(&tmp.path().join("project.yaml")).unwrap();
    let ownership = &project
        .artifact_graph
        .expect("artifact_graph")
        .code_ownership;
    assert!(ownership.contains_key("code-auth"));
    assert!(ownership.contains_key("tests-auth"));
    assert_eq!(ownership["code-auth"].owners, vec!["product"]);

    let audit = Command::new(hlv_bin())
        .args(["--root", tmp.path().to_str().unwrap(), "artifacts", "audit"])
        .output()
        .unwrap();
    assert!(
        audit.status.success(),
        "audit should pass after sync: {}",
        String::from_utf8_lossy(&audit.stderr)
    );
}
