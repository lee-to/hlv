//! Integration tests for adopted projects (HLV config under `.hlv/`).
//!
//! These use a minimal hand-written `.hlv/` layout with only currently
//! supported project.yaml fields. Full adopted fixtures with
//! `features.legacy_mode` / `paths.code` live in `tests/fixtures/adopt-*`.

use std::fs;
use std::path::Path;

use tempfile::TempDir;

use hlv::cmd::check::{get_check_report, CheckOptions};
use hlv::cmd::doctor::doctor_report;
use hlv::cmd::status::get_status;

/// Write a minimal adopted project: HLV artifacts under `root/.hlv/`,
/// observed code at the repository root.
fn write_minimal_adopted_project(root: &Path) {
    // Observed brownfield code at the repo root
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(root.join("app/main.py"), "def main():\n    pass\n").unwrap();
    fs::write(root.join("pyproject.toml"), "[project]\nname = \"demo\"\n").unwrap();

    // HLV config artifacts under .hlv/
    let hlv = root.join(".hlv");
    fs::create_dir_all(hlv.join("human/constraints")).unwrap();
    fs::create_dir_all(hlv.join("validation/scenarios")).unwrap();
    fs::create_dir_all(hlv.join("llm/src")).unwrap();
    fs::create_dir_all(hlv.join("llm/tests")).unwrap();
    fs::write(
        hlv.join("human/glossary.yaml"),
        "schema_version: 1\ntypes: {}\nenums: {}\n",
    )
    .unwrap();
    fs::write(
        hlv.join("validation/gates-policy.yaml"),
        "version: 1.0.0\npolicy_id: ADOPT-TEST\ngates: []\n",
    )
    .unwrap();
    fs::write(hlv.join("llm/map.yaml"), "schema_version: 1\nentries: []\n").unwrap();
    fs::write(
        hlv.join("milestones.yaml"),
        "project: adopted-demo\ncurrent:\nhistory: []\n",
    )
    .unwrap();
    fs::write(
        hlv.join("project.yaml"),
        r#"schema_version: 1
project: adopted-demo
status: draft
paths:
  human:
    glossary: human/glossary.yaml
    constraints: human/constraints/
  validation:
    gates_policy: validation/gates-policy.yaml
    scenarios: validation/scenarios/
  llm:
    src: llm/src/
    tests: llm/tests/
    map: llm/map.yaml
git:
  commit_convention: conventional
  merge_strategy: manual
"#,
    )
    .unwrap();
}

#[test]
fn find_project_root_discovers_adopted_layout_from_nested_dir() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    write_minimal_adopted_project(&root);

    let found = hlv::find_project_root_from(&root.join("app")).unwrap();
    assert_eq!(found.canonicalize().unwrap(), root);
    assert_eq!(hlv::config_root(&found), found.join(".hlv"));
}

#[test]
fn check_runs_against_adopted_layout() {
    let tmp = TempDir::new().unwrap();
    write_minimal_adopted_project(tmp.path());

    let report = get_check_report(tmp.path(), CheckOptions::default()).unwrap();
    // No fatal "project.yaml not found" — config was read from .hlv/
    assert!(
        !report.diagnostics.iter().any(|d| d.code == "PRJ-001"),
        "PRJ-001 should not fire for adopted layout: {:?}",
        report.diagnostics
    );
}

#[test]
fn status_runs_against_adopted_layout() {
    let tmp = TempDir::new().unwrap();
    write_minimal_adopted_project(tmp.path());

    let status = get_status(tmp.path()).unwrap();
    assert_eq!(status.project, "adopted-demo");
}

#[test]
fn doctor_runs_against_adopted_layout() {
    let tmp = TempDir::new().unwrap();
    write_minimal_adopted_project(tmp.path());

    let report = doctor_report(tmp.path(), false).unwrap();
    assert!(
        !report.diagnostics.iter().any(|d| d.code == "DOC-001"),
        "DOC-001 should not fire for adopted layout: {:?}",
        report.diagnostics
    );
    // Directory-existence checks must resolve under .hlv/, not the repo root
    assert!(
        !report.diagnostics.iter().any(|d| d.code == "DOC-021"),
        "DOC-021 should not fire when dirs exist under .hlv/: {:?}",
        report.diagnostics
    );
}

#[test]
fn workspace_add_accepts_adopted_layout() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path().join("workspace.yaml");
    let proj = tmp.path().join("adopted-proj");
    fs::create_dir(&proj).unwrap();
    write_minimal_adopted_project(&proj);

    hlv::cmd::workspace::run_init(Some(ws.to_str().unwrap())).unwrap();
    hlv::cmd::workspace::run_add(
        Some("adopted-proj"),
        Some(proj.to_str().unwrap()),
        Some(ws.to_str().unwrap()),
    )
    .unwrap();

    let config = hlv::mcp::workspace::WorkspaceConfig::load_lenient(&ws).unwrap();
    assert_eq!(config.projects.len(), 1);
    assert_eq!(config.projects[0].id, "adopted-proj");
}

#[test]
fn mcp_resource_read_works_for_adopted_layout() {
    let tmp = TempDir::new().unwrap();
    write_minimal_adopted_project(tmp.path());

    let result = hlv::mcp::resources::read_resource(tmp.path(), "hlv://project").unwrap();
    let text = match &result.contents[0] {
        rmcp::model::ResourceContents::TextResourceContents { text, .. } => text.clone(),
        other => panic!("expected text contents, got {other:?}"),
    };
    assert!(
        text.contains("adopted-demo"),
        "project resource should load from .hlv/: {text}"
    );
}

#[test]
fn adopt_init_writes_hlv_owned_files_under_hlv_dir() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();

    hlv::cmd::init::run_with_options(
        tmp.path().to_str().unwrap(),
        Some("adopted-init"),
        Some("team"),
        Some("claude"),
        Some("adopt-test"),
        Some("minimal"),
        true,
    )
    .unwrap();

    // HLV-owned files under .hlv/
    assert!(tmp.path().join(".hlv/project.yaml").exists());
    assert!(tmp.path().join(".hlv/milestones.yaml").exists());
    assert!(tmp.path().join(".hlv/human/glossary.yaml").exists());
    assert!(tmp.path().join(".hlv/validation/gates-policy.yaml").exists());
    assert!(tmp.path().join(".hlv/llm/map.yaml").exists());
    assert!(tmp.path().join(".hlv/schema/project-schema.json").exists());
    // Root-owned files stay at the repository root
    assert!(tmp.path().join("AGENTS.md").exists());
    assert!(tmp.path().join("HLV.md").exists());
    assert!(tmp.path().join(".claude/skills").is_dir());
    // No greenfield project.yaml at the root
    assert!(!tmp.path().join("project.yaml").exists());
}

#[test]
fn adopt_reinit_updates_hlv_layout_in_place() {
    let tmp = TempDir::new().unwrap();
    hlv::cmd::init::run_with_options(
        tmp.path().to_str().unwrap(),
        Some("adopted-init"),
        Some("team"),
        Some("claude"),
        Some("adopt-test"),
        Some("minimal"),
        true,
    )
    .unwrap();

    // Reinit (adopt flag not required — layout is auto-detected)
    hlv::cmd::init::run_with_options(
        tmp.path().to_str().unwrap(),
        None,
        None,
        None,
        None,
        None,
        false,
    )
    .unwrap();

    assert!(tmp.path().join(".hlv/project.yaml").exists());
    assert!(!tmp.path().join("project.yaml").exists());
}

#[test]
fn adopt_init_schema_comments_resolve_from_hlv_layout() {
    let tmp = TempDir::new().unwrap();
    hlv::cmd::init::run_with_options(
        tmp.path().to_str().unwrap(),
        Some("adopted-init"),
        Some("team"),
        Some("claude"),
        Some("adopt-test"),
        Some("minimal"),
        true,
    )
    .unwrap();

    // Every generated YAML with a $schema comment must point at an existing
    // schema file when resolved relative to the YAML's own directory.
    let mut checked = 0;
    let mut stack = vec![tmp.path().join(".hlv")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "yaml") {
                continue;
            }
            let content = fs::read_to_string(&path).unwrap();
            let Some(line) = content
                .lines()
                .find(|l| l.contains("yaml-language-server: $schema="))
            else {
                continue;
            };
            let rel = line.split("$schema=").nth(1).unwrap().trim();
            let resolved = path.parent().unwrap().join(rel);
            assert!(
                resolved.exists(),
                "$schema target missing for {}: {rel}",
                path.display()
            );
            checked += 1;
        }
    }
    assert!(checked >= 10, "expected many schema comments, got {checked}");
}

#[test]
fn greenfield_layout_still_prioritized_over_hlv_dir() {
    let tmp = TempDir::new().unwrap();
    write_minimal_adopted_project(tmp.path());
    // Add a root-level project.yaml — root layout must win
    fs::write(
        tmp.path().join("project.yaml"),
        fs::read(tmp.path().join(".hlv/project.yaml")).unwrap(),
    )
    .unwrap();

    assert_eq!(hlv::config_root(tmp.path()), tmp.path());
}
