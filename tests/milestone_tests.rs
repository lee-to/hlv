use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

use hlv::model::milestone::{
    GateResult, GateRunStatus, MilestoneMap, MilestoneStatus, StageStatus,
};

// ═══════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════

/// Create a minimal project scaffold for milestone tests.
fn setup_project(dir: &Path) -> &Path {
    // Minimal project.yaml
    fs::write(
        dir.join("project.yaml"),
        r#"schema_version: 1
project: test-project
status: draft
spec: schema/project-schema.json
paths:
  human:
    artifacts: human/artifacts/
    glossary: human/glossary.yaml
    constraints: human/constraints/
  validation:
    test_specs: validation/test-specs/
    scenarios: validation/scenarios/
    traceability: human/traceability.yaml
    gates_policy: validation/gates-policy.yaml
  llm:
    src: llm/src/
glossary_types: []
constraints: []
"#,
    )
    .unwrap();
    dir
}

fn load_milestones(dir: &Path) -> MilestoneMap {
    MilestoneMap::load(&dir.join("milestones.yaml")).unwrap()
}

// ═══════════════════════════════════════════════════════
// milestone new
// ═══════════════════════════════════════════════════════

#[test]
fn milestone_new_creates_structure() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::milestone::run_new(root, "order-create").unwrap();

    // milestones.yaml created
    assert!(root.join("milestones.yaml").exists());
    let m = load_milestones(root);
    let current = m.current.expect("should have current milestone");
    assert_eq!(current.id, "001-order-create");
    assert_eq!(current.number, 1);
    assert_eq!(current.branch, None); // branch creation not yet implemented
    assert!(current.stages.is_empty());

    // Directory structure created
    assert!(root
        .join("human/milestones/001-order-create/artifacts")
        .is_dir());
    assert!(root
        .join("human/milestones/001-order-create/contracts")
        .is_dir());
    assert!(root
        .join("human/milestones/001-order-create/test-specs")
        .is_dir());

    // plan.md created
    let plan = fs::read_to_string(root.join("human/milestones/001-order-create/plan.md")).unwrap();
    assert!(plan.contains("# Milestone: order-create"));
    assert!(plan.contains("## Stages"));
}

#[test]
fn milestone_new_autoincrement_number() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // Create and complete first milestone
    hlv::cmd::milestone::run_new(root, "first").unwrap();
    // Manually move to history (simulate done without stage validation)
    let mut m = load_milestones(root);
    let current = m.current.take().unwrap();
    m.history.push(hlv::model::milestone::HistoryEntry {
        id: current.id,
        number: current.number,
        status: MilestoneStatus::Merged,
        contracts: vec![],
        branch: current.branch,
        merged_at: Some("2026-03-01".to_string()),
    });
    let yaml = format!(
        "# yaml-language-server: $schema=schema/milestones-schema.json\n{}",
        serde_yaml::to_string(&m).unwrap()
    );
    fs::write(root.join("milestones.yaml"), yaml).unwrap();

    // Create second milestone — should be 002
    hlv::cmd::milestone::run_new(root, "second").unwrap();
    let m = load_milestones(root);
    let current = m.current.unwrap();
    assert_eq!(current.number, 2);
    assert_eq!(current.id, "002-second");
}

#[test]
fn milestone_new_fails_if_active_exists() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::milestone::run_new(root, "first").unwrap();
    let result = hlv::cmd::milestone::run_new(root, "second");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Active milestone exists"));
}

// ═══════════════════════════════════════════════════════
// milestone status
// ═══════════════════════════════════════════════════════

#[test]
fn milestone_status_no_active() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    fs::write(
        root.join("milestones.yaml"),
        "project: test-project\nhistory: []\n",
    )
    .unwrap();

    // Should not error, just show "no active milestone"
    hlv::cmd::milestone::run_status(root).unwrap();
}

#[test]
fn milestone_status_with_active() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::milestone::run_new(root, "my-feature").unwrap();
    hlv::cmd::milestone::run_status(root).unwrap();
}

// ═══════════════════════════════════════════════════════
// milestone list
// ═══════════════════════════════════════════════════════

#[test]
fn milestone_list_empty() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    fs::write(
        root.join("milestones.yaml"),
        "project: test-project\nhistory: []\n",
    )
    .unwrap();

    hlv::cmd::milestone::run_list(root).unwrap();
}

// ═══════════════════════════════════════════════════════
// milestone done
// ═══════════════════════════════════════════════════════

#[test]
fn milestone_done_all_validated() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // Create milestone with stages and set all to validated
    hlv::cmd::milestone::run_new(root, "done-test").unwrap();
    let mut m = load_milestones(root);
    let current = m.current.as_mut().unwrap();
    current.stages = vec![
        hlv::model::milestone::StageEntry {
            id: 1,
            scope: "Foundation".to_string(),
            status: StageStatus::Validated,
            commit: Some("abc1234".to_string()),
            tasks: Vec::new(),
            labels: Vec::new(),
            meta: std::collections::HashMap::new(),
        },
        hlv::model::milestone::StageEntry {
            id: 2,
            scope: "Integration".to_string(),
            status: StageStatus::Validated,
            commit: Some("def5678".to_string()),
            tasks: Vec::new(),
            labels: Vec::new(),
            meta: std::collections::HashMap::new(),
        },
    ];
    // Create contracts dir with a contract file
    let contracts_dir = root.join("human/milestones/001-done-test/contracts");
    fs::create_dir_all(&contracts_dir).unwrap();
    fs::write(contracts_dir.join("order.create.md"), "# Contract").unwrap();
    fs::write(contracts_dir.join("order.create.yaml"), "id: order.create").unwrap();

    let yaml = format!(
        "# yaml-language-server: $schema=schema/milestones-schema.json\n{}",
        serde_yaml::to_string(&m).unwrap()
    );
    fs::write(root.join("milestones.yaml"), yaml).unwrap();

    hlv::cmd::milestone::run_done(root).unwrap();

    let m = load_milestones(root);
    assert!(m.current.is_none());
    assert_eq!(m.history.len(), 1);
    assert_eq!(m.history[0].id, "001-done-test");
    assert_eq!(m.history[0].status, MilestoneStatus::Merged);
    assert!(!m.history[0].contracts.is_empty());
}

#[test]
fn milestone_done_fails_if_unvalidated_stages() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::milestone::run_new(root, "incomplete").unwrap();
    let mut m = load_milestones(root);
    let current = m.current.as_mut().unwrap();
    current.stages = vec![hlv::model::milestone::StageEntry {
        id: 1,
        scope: "Foundation".to_string(),
        status: StageStatus::Implementing,
        commit: None,
        tasks: Vec::new(),
        labels: Vec::new(),
        meta: std::collections::HashMap::new(),
    }];
    let yaml = format!(
        "# yaml-language-server: $schema=schema/milestones-schema.json\n{}",
        serde_yaml::to_string(&m).unwrap()
    );
    fs::write(root.join("milestones.yaml"), yaml).unwrap();

    let result = hlv::cmd::milestone::run_done(root);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not validated"));
}

#[test]
fn milestone_done_fails_if_no_active() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    fs::write(
        root.join("milestones.yaml"),
        "project: test-project\nhistory: []\n",
    )
    .unwrap();

    let result = hlv::cmd::milestone::run_done(root);
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════
// milestone abort
// ═══════════════════════════════════════════════════════

#[test]
fn milestone_abort_moves_to_history() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::milestone::run_new(root, "aborted-feature").unwrap();
    hlv::cmd::milestone::run_abort(root).unwrap();

    let m = load_milestones(root);
    assert!(m.current.is_none());
    assert_eq!(m.history.len(), 1);
    assert_eq!(m.history[0].status, MilestoneStatus::Aborted);
    assert_eq!(m.history[0].id, "001-aborted-feature");
}

#[test]
fn milestone_abort_fails_if_no_active() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    fs::write(
        root.join("milestones.yaml"),
        "project: test-project\nhistory: []\n",
    )
    .unwrap();

    let result = hlv::cmd::milestone::run_abort(root);
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════
// milestone slug generation
// ═══════════════════════════════════════════════════════

#[test]
fn milestone_slug_special_chars() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::milestone::run_new(root, "Add Payment Method!").unwrap();
    let m = load_milestones(root);
    let current = m.current.unwrap();
    assert_eq!(current.id, "001-add-payment-method");
}

// ═══════════════════════════════════════════════════════
// gate results persistence
// ═══════════════════════════════════════════════════════

#[test]
fn gate_results_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::milestone::run_new(root, "gates-test").unwrap();

    // Add gate results manually
    let ms_path = root.join("milestones.yaml");
    let mut m = MilestoneMap::load(&ms_path).unwrap();
    let current = m.current.as_mut().unwrap();
    current.gate_results = vec![
        GateResult {
            id: "unit-tests".to_string(),
            status: GateRunStatus::Passed,
            run_at: Some("2026-03-06T12:00:00+00:00".to_string()),
        },
        GateResult {
            id: "lint".to_string(),
            status: GateRunStatus::Failed,
            run_at: Some("2026-03-06T12:00:00+00:00".to_string()),
        },
    ];
    m.save(&ms_path).unwrap();

    // Reload and verify
    let m2 = MilestoneMap::load(&ms_path).unwrap();
    let current2 = m2.current.unwrap();
    assert_eq!(current2.gate_results.len(), 2);
    assert_eq!(current2.gate_results[0].id, "unit-tests");
    assert_eq!(current2.gate_results[0].status, GateRunStatus::Passed);
    assert_eq!(current2.gate_results[1].id, "lint");
    assert_eq!(current2.gate_results[1].status, GateRunStatus::Failed);
}

#[test]
fn gate_results_empty_not_serialized() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::milestone::run_new(root, "no-gates").unwrap();

    let content = fs::read_to_string(root.join("milestones.yaml")).unwrap();
    // gate_results should not appear when empty (skip_serializing_if)
    assert!(
        !content.contains("gate_results"),
        "empty gate_results should not be serialized"
    );
}

#[test]
fn gate_results_survives_milestone_reload() {
    // Verify gate_results doesn't break existing milestones.yaml without the field
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    fs::write(
        root.join("milestones.yaml"),
        r#"project: test-project
current:
  id: 001-legacy
  number: 1
  stages: []
history: []
"#,
    )
    .unwrap();

    let m = MilestoneMap::load(&root.join("milestones.yaml")).unwrap();
    let current = m.current.unwrap();
    assert!(current.gate_results.is_empty());
}

// ═══════════════════════════════════════════════════════
// git integration
// ═══════════════════════════════════════════════════════

fn setup_git_project(dir: &Path) {
    setup_project(dir);
    // Init git repo with main branch
    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(dir)
        .output()
        .unwrap();
}

fn setup_git_project_with_branching(dir: &Path) {
    setup_git_project(dir);
    // Update project.yaml with branch_per_milestone: true
    let yaml = r#"schema_version: 1
project: test-project
status: draft
paths:
  human:
    artifacts: human/artifacts/
    glossary: human/glossary.yaml
    constraints: human/constraints/
  validation:
    test_specs: validation/test-specs/
    scenarios: validation/scenarios/
    traceability: human/traceability.yaml
    gates_policy: validation/gates-policy.yaml
  llm:
    src: llm/src/
glossary_types: []
constraints: []
git:
  branch_per_milestone: true
  branch_format: "feature/{milestone-slug}"
  commit_convention: conventional
  merge_strategy: manual
"#;
    fs::write(dir.join("project.yaml"), yaml).unwrap();
}

fn git_current_branch(dir: &Path) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn milestone_new_creates_branch_when_enabled() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_git_project_with_branching(root);

    hlv::cmd::milestone::run_new(root, "payment").unwrap();

    // Should be on the new branch
    assert_eq!(git_current_branch(root), "feature/001-payment");

    // Milestone should record the branch
    let m = load_milestones(root);
    let current = m.current.unwrap();
    assert_eq!(current.branch.as_deref(), Some("feature/001-payment"));
}

#[test]
fn milestone_new_no_branch_when_disabled() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_git_project(root);
    // Default project.yaml has no git section → branch_per_milestone defaults to false

    hlv::cmd::milestone::run_new(root, "no-branch").unwrap();

    // Should stay on main
    assert_eq!(git_current_branch(root), "main");

    let m = load_milestones(root);
    let current = m.current.unwrap();
    assert_eq!(current.branch, None);
}
