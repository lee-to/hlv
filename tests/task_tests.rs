use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

use hlv::model::milestone::{MilestoneMap, StageEntry, StageStatus};
use hlv::model::task::{TaskStatus, TaskTracker};

// ═══════════════════════════════════════════════════════
// P4.2 — Schema validation tests for TaskTracker
// ═══════════════════════════════════════════════════════

fn validate_yaml_against_schema(yaml_str: &str, schema_name: &str) -> Vec<String> {
    let schema_path = Path::new("schema").join(schema_name);
    let schema_str = fs::read_to_string(&schema_path).unwrap();
    let schema: Value = serde_json::from_str(&schema_str).unwrap();

    let yaml_val: serde_yaml::Value = serde_yaml::from_str(yaml_str).unwrap();
    let json_val: Value = serde_json::to_value(yaml_val).unwrap();

    let validator = jsonschema::validator_for(&schema).unwrap();
    validator
        .iter_errors(&json_val)
        .map(|e| format!("{}: {}", e.instance_path, e))
        .collect()
}

#[test]
fn milestones_with_tasks_validates_against_schema() {
    let yaml = r#"
project: test
current:
  id: "001-test"
  number: 1
  stages:
    - id: 1
      scope: "Foundation"
      status: implementing
      tasks:
        - id: TASK-001
          status: in_progress
          started_at: "2026-03-08T10:00:00Z"
          labels: ["frontend"]
          meta:
            priority: high
        - id: TASK-002
          status: pending
        - id: FIX-001
          status: done
          started_at: "2026-03-07T09:00:00Z"
          completed_at: "2026-03-08T11:00:00Z"
      labels: ["sprint-3"]
      meta:
        assignee: "@vasya"
  labels: ["backend"]
  meta:
    owner: "@lead"
"#;
    let errors = validate_yaml_against_schema(yaml, "milestones-schema.json");
    assert!(errors.is_empty(), "Validation errors: {:?}", errors);
}

#[test]
fn milestones_without_tasks_validates_against_schema() {
    let yaml = r#"
project: test
current:
  id: "001-test"
  number: 1
  stages:
    - id: 1
      scope: "Foundation"
      status: pending
"#;
    let errors = validate_yaml_against_schema(yaml, "milestones-schema.json");
    assert!(errors.is_empty(), "Validation errors: {:?}", errors);
}

#[test]
fn task_tracker_with_labels_meta_validates() {
    let yaml = r#"
project: test
current:
  id: "001-test"
  number: 1
  stages:
    - id: 1
      scope: "Test"
      status: implementing
      tasks:
        - id: TASK-001
          status: blocked
          block_reason: "waiting for API access"
          labels: ["needs-review", "frontend"]
          meta:
            reviewer: "@vasya"
            priority: "high"
            deployed_at: "2026-03-08"
"#;
    let errors = validate_yaml_against_schema(yaml, "milestones-schema.json");
    assert!(errors.is_empty(), "Validation errors: {:?}", errors);
}

#[test]
fn task_tracker_invalid_status_fails_validation() {
    let yaml = r#"
project: test
current:
  id: "001-test"
  number: 1
  stages:
    - id: 1
      scope: "Test"
      status: implementing
      tasks:
        - id: TASK-001
          status: invalid_status
"#;
    let errors = validate_yaml_against_schema(yaml, "milestones-schema.json");
    assert!(
        !errors.is_empty(),
        "Should have validation errors for invalid task status"
    );
    assert!(
        errors.iter().any(|e| e.contains("invalid_status")),
        "Error should mention invalid_status, got: {:?}",
        errors
    );
}

// ═══════════════════════════════════════════════════════
// P4.3 — Integration tests
// ═══════════════════════════════════════════════════════

fn setup_project(root: &Path) {
    hlv::cmd::init::run_with_milestone(
        root.to_str().unwrap(),
        Some("test-proj"),
        Some("team"),
        Some("claude"),
        Some("task-test"),
        Some("minimal"),
    )
    .unwrap();
}

fn load_milestones(root: &Path) -> MilestoneMap {
    MilestoneMap::load(&root.join("milestones.yaml")).unwrap()
}

fn save_milestones(root: &Path, map: &MilestoneMap) {
    map.save(&root.join("milestones.yaml")).unwrap();
}

#[test]
fn task_lifecycle_integration() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // Add stages with tasks to milestones.yaml
    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Foundation".to_string(),
        status: StageStatus::Pending,
        commit: None,
        tasks: vec![
            TaskTracker::new("TASK-001".to_string()),
            TaskTracker::new("TASK-002".to_string()),
        ],
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    // Start TASK-001
    hlv::cmd::task::run_start(root, "TASK-001").unwrap();
    let map = load_milestones(root);
    let stage = &map.current.as_ref().unwrap().stages[0];
    assert_eq!(stage.tasks[0].status, TaskStatus::InProgress);
    assert!(stage.tasks[0].started_at.is_some());
    // Stage should auto-transition to Implementing
    assert_eq!(stage.status, StageStatus::Implementing);

    // Done TASK-001
    hlv::cmd::task::run_done(root, "TASK-001").unwrap();
    let map = load_milestones(root);
    let task = &map.current.as_ref().unwrap().stages[0].tasks[0];
    assert_eq!(task.status, TaskStatus::Done);
    assert!(task.completed_at.is_some());

    // Block TASK-002
    hlv::cmd::task::run_block(root, "TASK-002", "waiting for infra").unwrap();
    let map = load_milestones(root);
    let task = &map.current.as_ref().unwrap().stages[0].tasks[1];
    assert_eq!(task.status, TaskStatus::Blocked);
    assert_eq!(task.block_reason.as_deref(), Some("waiting for infra"));

    // Unblock TASK-002
    hlv::cmd::task::run_unblock(root, "TASK-002").unwrap();
    let map = load_milestones(root);
    let task = &map.current.as_ref().unwrap().stages[0].tasks[1];
    assert_eq!(task.status, TaskStatus::Pending);
    assert!(task.block_reason.is_none());

    // Label and meta
    hlv::cmd::task::run_label(root, "TASK-002", "add", "frontend").unwrap();
    hlv::cmd::task::run_meta(root, "TASK-002", "set", "priority", Some("high")).unwrap();
    let map = load_milestones(root);
    let task = &map.current.as_ref().unwrap().stages[0].tasks[1];
    assert_eq!(task.labels, vec!["frontend"]);
    assert_eq!(task.meta.get("priority").unwrap(), "high");
}

#[test]
fn task_sync_integration() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // Add a stage to milestones
    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Foundation".to_string(),
        status: StageStatus::Pending,
        commit: None,
        tasks: Vec::new(),
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    // Create stage_1.md
    let milestone_id = load_milestones(root).current.unwrap().id;
    let stage_dir = root.join("human/milestones").join(&milestone_id);
    fs::write(
        stage_dir.join("stage_1.md"),
        r#"# Stage 1: Foundation

## Tasks

TASK-001 Domain Types
  contracts: [order.create]
  output: llm/src/domain/

TASK-002 Handler
  depends_on: [TASK-001]
  contracts: [order.create]
  output: llm/src/handler/
"#,
    )
    .unwrap();

    // Sync
    hlv::cmd::task::run_sync(root, false).unwrap();
    let map = load_milestones(root);
    let tasks = &map.current.as_ref().unwrap().stages[0].tasks;
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].id, "TASK-001");
    assert_eq!(tasks[0].status, TaskStatus::Pending);
    assert_eq!(tasks[1].id, "TASK-002");
}

#[test]
fn task_start_checks_dependencies() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let milestone_id = load_milestones(root).current.unwrap().id;
    let stage_dir = root.join("human/milestones").join(&milestone_id);

    // Create stage_1.md with dependency
    fs::write(
        stage_dir.join("stage_1.md"),
        r#"# Stage 1: Test

## Tasks

TASK-001 First
  output: llm/src/a/

TASK-002 Second
  depends_on: [TASK-001]
  output: llm/src/b/
"#,
    )
    .unwrap();

    // Add stage with tasks
    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Test".to_string(),
        status: StageStatus::Pending,
        commit: None,
        tasks: vec![
            TaskTracker::new("TASK-001".to_string()),
            TaskTracker::new("TASK-002".to_string()),
        ],
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    // Try to start TASK-002 — should fail (TASK-001 not done)
    let result = hlv::cmd::task::run_start(root, "TASK-002");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("dependency TASK-001 is not done"));

    // Start and complete TASK-001
    hlv::cmd::task::run_start(root, "TASK-001").unwrap();
    hlv::cmd::task::run_done(root, "TASK-001").unwrap();

    // Now TASK-002 should start
    hlv::cmd::task::run_start(root, "TASK-002").unwrap();
    let map = load_milestones(root);
    assert_eq!(
        map.current.as_ref().unwrap().stages[0].tasks[1].status,
        TaskStatus::InProgress
    );
}

#[test]
fn stage_and_milestone_labels_meta() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // Add a stage
    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Test".to_string(),
        status: StageStatus::Pending,
        commit: None,
        tasks: Vec::new(),
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    // Stage label/meta
    hlv::cmd::stage::run_label(root, 1, "add", "backend").unwrap();
    hlv::cmd::stage::run_meta(root, 1, "set", "deadline", Some("2026-03-15")).unwrap();

    let map = load_milestones(root);
    let stage = &map.current.as_ref().unwrap().stages[0];
    assert_eq!(stage.labels, vec!["backend"]);
    assert_eq!(stage.meta.get("deadline").unwrap(), "2026-03-15");

    // Milestone label/meta
    hlv::cmd::stage::run_milestone_label(root, "add", "q2-2026").unwrap();
    hlv::cmd::stage::run_milestone_meta(root, "set", "owner", Some("@lead")).unwrap();

    let map = load_milestones(root);
    let current = map.current.as_ref().unwrap();
    assert_eq!(current.labels, vec!["q2-2026"]);
    assert_eq!(current.meta.get("owner").unwrap(), "@lead");

    // Remove
    hlv::cmd::stage::run_label(root, 1, "remove", "backend").unwrap();
    hlv::cmd::stage::run_milestone_label(root, "remove", "q2-2026").unwrap();

    let map = load_milestones(root);
    assert!(map.current.as_ref().unwrap().stages[0].labels.is_empty());
    assert!(map.current.as_ref().unwrap().labels.is_empty());
}

#[test]
fn json_output_smoke_test() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // Verify get_status returns valid data
    let status = hlv::cmd::status::get_status(root).unwrap();
    let json = serde_json::to_string(&status).unwrap();
    let parsed: Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.get("project").is_some());

    // Verify get_workflow returns valid data
    let workflow = hlv::cmd::workflow::get_workflow(root).unwrap();
    let json = serde_json::to_string(&workflow).unwrap();
    let parsed: Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.get("phase").is_some());

    // Verify get_plan returns valid data
    let plan = hlv::cmd::plan::get_plan(root).unwrap();
    let json = serde_json::to_string(&plan).unwrap();
    let _: Value = serde_json::from_str(&json).unwrap();

    // Verify get_check_diagnostics returns valid data
    let (diags, code) = hlv::cmd::check::get_check_diagnostics(root).unwrap();
    assert!(code == 0 || code == 1); // valid exit code
    for d in &diags {
        assert!(!d.code.is_empty());
    }
}

#[test]
fn backward_compatibility_milestones_without_tasks() {
    // YAML without tasks field should deserialize with tasks = []
    let yaml = r#"
project: test
current:
  id: "001-test"
  number: 1
  stages:
    - id: 1
      scope: "Foundation"
      status: pending
"#;
    let map: MilestoneMap = serde_yaml::from_str(yaml).unwrap();
    let stage = &map.current.as_ref().unwrap().stages[0];
    assert!(stage.tasks.is_empty());
    assert!(stage.labels.is_empty());
    assert!(stage.meta.is_empty());
}

#[test]
fn backward_compatibility_milestones_with_tasks() {
    let yaml = r#"
project: test
current:
  id: "001-test"
  number: 1
  stages:
    - id: 1
      scope: "Foundation"
      status: implementing
      tasks:
        - id: TASK-001
          status: in_progress
          started_at: "2026-03-08T10:00:00Z"
        - id: TASK-002
          status: pending
"#;
    let map: MilestoneMap = serde_yaml::from_str(yaml).unwrap();
    let stage = &map.current.as_ref().unwrap().stages[0];
    assert_eq!(stage.tasks.len(), 2);
    assert_eq!(stage.tasks[0].status, TaskStatus::InProgress);
    assert_eq!(stage.tasks[1].status, TaskStatus::Pending);
}

// ═══════════════════════════════════════════════════════
// P4.3 — TSK-* diagnostics integration test
// ═══════════════════════════════════════════════════════

#[test]
fn tsk_diagnostics_integration() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let milestone_id = load_milestones(root).current.unwrap().id;
    let stage_dir = root.join("human/milestones").join(&milestone_id);

    // Create stage_1.md with tasks and outputs
    fs::write(
        stage_dir.join("stage_1.md"),
        r#"# Stage 1: Foundation

## Tasks

TASK-001 Domain Types
  output: llm/src/domain/

TASK-002 Handler
  depends_on: [TASK-001]
  output: llm/src/handler/

TASK-003 Extra
  output: llm/src/extra/
"#,
    )
    .unwrap();

    // Set up milestones with specific task states to trigger diagnostics
    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Foundation".to_string(),
        status: StageStatus::Implementing,
        commit: None,
        tasks: vec![
            // TASK-001: Done but output doesn't exist → TSK-020
            {
                let mut t = TaskTracker::new("TASK-001".to_string());
                t.status = TaskStatus::Done;
                t.started_at = Some("2026-03-07T09:00:00Z".to_string());
                t.completed_at = Some("2026-03-08T10:00:00Z".to_string());
                t
            },
            // TASK-002: InProgress but dep TASK-001 is done — no TSK-040
            {
                let mut t = TaskTracker::new("TASK-002".to_string());
                t.status = TaskStatus::InProgress;
                t.started_at = Some("2026-03-08T11:00:00Z".to_string());
                t
            },
            // TASK-003: Done
            {
                let mut t = TaskTracker::new("TASK-003".to_string());
                t.status = TaskStatus::Done;
                t.started_at = Some("2026-03-07T08:00:00Z".to_string());
                t.completed_at = Some("2026-03-08T12:00:00Z".to_string());
                t
            },
            // ORPHAN-001: in tracker but not in stage_1.md → TSK-050
            TaskTracker::new("ORPHAN-001".to_string()),
        ],
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    // Run check diagnostics
    let (diags, _code) = hlv::cmd::check::get_check_diagnostics(root).unwrap();

    // TSK-020: TASK-001 done but llm/src/domain/ doesn't exist
    assert!(
        diags
            .iter()
            .any(|d| d.code == "TSK-020" && d.message.contains("TASK-001")),
        "Expected TSK-020 for TASK-001, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.starts_with("TSK"))
            .collect::<Vec<_>>()
    );

    // TSK-020: TASK-003 done but llm/src/extra/ doesn't exist
    assert!(
        diags
            .iter()
            .any(|d| d.code == "TSK-020" && d.message.contains("TASK-003")),
        "Expected TSK-020 for TASK-003"
    );

    // TSK-050: ORPHAN-001 not in stage plan
    assert!(
        diags
            .iter()
            .any(|d| d.code == "TSK-050" && d.message.contains("ORPHAN-001")),
        "Expected TSK-050 for ORPHAN-001"
    );

    // No TSK-040 — TASK-002 depends on TASK-001 which IS done
    assert!(
        !diags
            .iter()
            .any(|d| d.code == "TSK-040" && d.message.contains("TASK-002")),
        "Should NOT have TSK-040 for TASK-002 (dep is done)"
    );
}

// ═══════════════════════════════════════════════════════
// Stage reopen tests
// ═══════════════════════════════════════════════════════

#[test]
fn stage_reopen_implemented_to_implementing() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Foundation".to_string(),
        status: StageStatus::Implemented,
        commit: None,
        tasks: Vec::new(),
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    hlv::cmd::stage::run_reopen(root, 1).unwrap();

    let map = load_milestones(root);
    let stage = &map.current.as_ref().unwrap().stages[0];
    assert_eq!(stage.status, StageStatus::Implementing);
    assert_eq!(map.current.as_ref().unwrap().stage, Some(1));
}

#[test]
fn stage_reopen_validated_to_validating() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Foundation".to_string(),
        status: StageStatus::Validated,
        commit: None,
        tasks: Vec::new(),
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    hlv::cmd::stage::run_reopen(root, 1).unwrap();

    let map = load_milestones(root);
    let stage = &map.current.as_ref().unwrap().stages[0];
    assert_eq!(stage.status, StageStatus::Validating);
}

#[test]
fn stage_reopen_validating_to_implementing() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Foundation".to_string(),
        status: StageStatus::Validating,
        commit: None,
        tasks: Vec::new(),
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    hlv::cmd::stage::run_reopen(root, 1).unwrap();

    let map = load_milestones(root);
    let stage = &map.current.as_ref().unwrap().stages[0];
    assert_eq!(stage.status, StageStatus::Implementing);
}

#[test]
fn stage_reopen_pending_fails() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Foundation".to_string(),
        status: StageStatus::Pending,
        commit: None,
        tasks: Vec::new(),
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    let result = hlv::cmd::stage::run_reopen(root, 1);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Cannot reopen"));
}

#[test]
fn stage_reopen_implementing_fails() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Foundation".to_string(),
        status: StageStatus::Implementing,
        commit: None,
        tasks: Vec::new(),
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    let result = hlv::cmd::stage::run_reopen(root, 1);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Cannot reopen"));
}

#[test]
fn stage_reopen_nonexistent_fails() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::cmd::stage::run_reopen(root, 99);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Stage 99 not found"));
}

// ═══════════════════════════════════════════════════════
// Task add tests
// ═══════════════════════════════════════════════════════

#[test]
fn task_add_to_pending_stage() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let milestone_id = load_milestones(root).current.unwrap().id;
    let stage_dir = root.join("human/milestones").join(&milestone_id);

    fs::write(
        stage_dir.join("stage_1.md"),
        "# Stage 1: Foundation\n\n## Tasks\n\nTASK-001 First\n  contracts: []\n\n## Remediation\n",
    )
    .unwrap();

    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Foundation".to_string(),
        status: StageStatus::Pending,
        commit: None,
        tasks: vec![TaskTracker::new("TASK-001".to_string())],
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    hlv::cmd::task::run_add(root, 1, "TASK-002", "New feature").unwrap();

    let map = load_milestones(root);
    let stage = &map.current.as_ref().unwrap().stages[0];
    assert_eq!(stage.tasks.len(), 2);
    assert_eq!(stage.tasks[1].id, "TASK-002");
    assert_eq!(stage.tasks[1].status, TaskStatus::Pending);

    // Check stage_1.md was updated
    let content = fs::read_to_string(stage_dir.join("stage_1.md")).unwrap();
    assert!(content.contains("TASK-002 New feature"));
    // Task should be inserted before ## Remediation
    let task_pos = content.find("TASK-002").unwrap();
    let rem_pos = content.find("## Remediation").unwrap();
    assert!(
        task_pos < rem_pos,
        "Task should be before Remediation section"
    );
}

#[test]
fn task_add_auto_reopens_implemented_stage() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let milestone_id = load_milestones(root).current.unwrap().id;
    let stage_dir = root.join("human/milestones").join(&milestone_id);

    fs::write(
        stage_dir.join("stage_1.md"),
        "# Stage 1: Foundation\n\n## Tasks\n\nTASK-001 First\n  contracts: []\n\n## Remediation\n",
    )
    .unwrap();

    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Foundation".to_string(),
        status: StageStatus::Implemented,
        commit: None,
        tasks: vec![{
            let mut t = TaskTracker::new("TASK-001".to_string());
            t.status = TaskStatus::Done;
            t
        }],
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    hlv::cmd::task::run_add(root, 1, "TASK-002", "Fix bug").unwrap();

    let map = load_milestones(root);
    let stage = &map.current.as_ref().unwrap().stages[0];
    assert_eq!(
        stage.status,
        StageStatus::Implementing,
        "Should auto-reopen to implementing"
    );
    assert_eq!(map.current.as_ref().unwrap().stage, Some(1));
    assert_eq!(stage.tasks.len(), 2);
}

#[test]
fn task_add_duplicate_fails() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let milestone_id = load_milestones(root).current.unwrap().id;
    let stage_dir = root.join("human/milestones").join(&milestone_id);

    fs::write(
        stage_dir.join("stage_1.md"),
        "# Stage 1: Foundation\n\n## Tasks\n\nTASK-001 First\n  contracts: []\n",
    )
    .unwrap();

    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Foundation".to_string(),
        status: StageStatus::Pending,
        commit: None,
        tasks: vec![TaskTracker::new("TASK-001".to_string())],
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    let result = hlv::cmd::task::run_add(root, 1, "TASK-001", "Duplicate");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));
}

#[test]
fn task_add_invalid_id_fails() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let milestone_id = load_milestones(root).current.unwrap().id;
    let stage_dir = root.join("human/milestones").join(&milestone_id);

    fs::write(
        stage_dir.join("stage_1.md"),
        "# Stage 1: Foundation\n\n## Tasks\n",
    )
    .unwrap();

    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Foundation".to_string(),
        status: StageStatus::Pending,
        commit: None,
        tasks: Vec::new(),
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    let result = hlv::cmd::task::run_add(root, 1, "BAD-001", "Wrong prefix");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("TASK- or FIX-"));
}

#[test]
fn task_add_fix_id_works() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let milestone_id = load_milestones(root).current.unwrap().id;
    let stage_dir = root.join("human/milestones").join(&milestone_id);

    fs::write(
        stage_dir.join("stage_1.md"),
        "# Stage 1: Foundation\n\n## Tasks\n\nTASK-001 First\n  contracts: []\n\n## Remediation\n",
    )
    .unwrap();

    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Foundation".to_string(),
        status: StageStatus::Validating,
        commit: None,
        tasks: vec![TaskTracker::new("TASK-001".to_string())],
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    hlv::cmd::task::run_add(root, 1, "FIX-001", "Fix validation error").unwrap();

    let map = load_milestones(root);
    let stage = &map.current.as_ref().unwrap().stages[0];
    assert_eq!(stage.tasks.len(), 2);
    assert_eq!(stage.tasks[1].id, "FIX-001");
    assert_eq!(
        stage.status,
        StageStatus::Implementing,
        "Validating should auto-reopen"
    );
}

// ═══════════════════════════════════════════════════════
// Issue #1 — Cross-stage dependency tests
// ═══════════════════════════════════════════════════════

#[test]
fn cross_stage_dependency_start_allowed() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let milestone_id = load_milestones(root).current.unwrap().id;
    let stage_dir = root.join("human/milestones").join(&milestone_id);

    // Stage 1: TASK-001
    fs::write(
        stage_dir.join("stage_1.md"),
        "# Stage 1: Base\n\n## Tasks\n\nTASK-001 Foundation\n  output: llm/src/a/\n",
    )
    .unwrap();

    // Stage 2: TASK-002 depends on TASK-001 from stage 1
    fs::write(
        stage_dir.join("stage_2.md"),
        "# Stage 2: Extension\n\n## Tasks\n\nTASK-002 Extends\n  depends_on: [TASK-001]\n  output: llm/src/b/\n",
    )
    .unwrap();

    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![
        StageEntry {
            id: 1,
            scope: "Base".to_string(),
            status: StageStatus::Implementing,
            commit: None,
            tasks: vec![TaskTracker::new("TASK-001".to_string())],
            labels: Vec::new(),
            meta: HashMap::new(),
        },
        StageEntry {
            id: 2,
            scope: "Extension".to_string(),
            status: StageStatus::Pending,
            commit: None,
            tasks: vec![TaskTracker::new("TASK-002".to_string())],
            labels: Vec::new(),
            meta: HashMap::new(),
        },
    ];
    save_milestones(root, &map);

    // TASK-002 should fail — TASK-001 not done yet
    let result = hlv::cmd::task::run_start(root, "TASK-002");
    assert!(result.is_err(), "Should fail: cross-stage dep not done");

    // Complete TASK-001
    hlv::cmd::task::run_start(root, "TASK-001").unwrap();
    hlv::cmd::task::run_done(root, "TASK-001").unwrap();

    // Now TASK-002 should succeed
    hlv::cmd::task::run_start(root, "TASK-002").unwrap();
    let map = load_milestones(root);
    assert_eq!(
        map.current.as_ref().unwrap().stages[1].tasks[0].status,
        TaskStatus::InProgress
    );
}

#[test]
fn tsk040_cross_stage_no_false_positive() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let milestone_id = load_milestones(root).current.unwrap().id;
    let stage_dir = root.join("human/milestones").join(&milestone_id);

    fs::write(
        stage_dir.join("stage_1.md"),
        "# Stage 1: Base\n\n## Tasks\n\nTASK-001 Foundation\n  output: llm/src/a/\n",
    )
    .unwrap();
    fs::write(
        stage_dir.join("stage_2.md"),
        "# Stage 2: Extension\n\n## Tasks\n\nTASK-002 Extends\n  depends_on: [TASK-001]\n  output: llm/src/b/\n",
    )
    .unwrap();

    // TASK-001 is Done in stage 1, TASK-002 is InProgress in stage 2
    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![
        StageEntry {
            id: 1,
            scope: "Base".to_string(),
            status: StageStatus::Implementing,
            commit: None,
            tasks: vec![{
                let mut t = TaskTracker::new("TASK-001".to_string());
                t.status = TaskStatus::Done;
                t.completed_at = Some("2026-03-08T10:00:00Z".to_string());
                t
            }],
            labels: Vec::new(),
            meta: HashMap::new(),
        },
        StageEntry {
            id: 2,
            scope: "Extension".to_string(),
            status: StageStatus::Implementing,
            commit: None,
            tasks: vec![{
                let mut t = TaskTracker::new("TASK-002".to_string());
                t.status = TaskStatus::InProgress;
                t.started_at = Some("2026-03-08T11:00:00Z".to_string());
                t
            }],
            labels: Vec::new(),
            meta: HashMap::new(),
        },
    ];
    save_milestones(root, &map);

    let (diags, _) = hlv::cmd::check::get_check_diagnostics(root).unwrap();
    // Should NOT get TSK-040 for TASK-002 — its dep TASK-001 is done (in another stage)
    assert!(
        !diags
            .iter()
            .any(|d| d.code == "TSK-040" && d.message.contains("TASK-002")),
        "False positive TSK-040 for cross-stage dep, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code == "TSK-040")
            .collect::<Vec<_>>()
    );
}

// ═══════════════════════════════════════════════════════
// Issue #2 — Sync discovers stage files and creates entries
// ═══════════════════════════════════════════════════════

#[test]
fn task_sync_discovers_new_stages() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let milestone_id = load_milestones(root).current.unwrap().id;
    let stage_dir = root.join("human/milestones").join(&milestone_id);

    // Milestone starts with empty stages (as created by `hlv milestone new`)
    let map = load_milestones(root);
    assert!(map.current.as_ref().unwrap().stages.is_empty());

    // Create stage files
    fs::write(
        stage_dir.join("stage_1.md"),
        "# Stage 1: Foundation\n\n## Tasks\n\nTASK-001 Types\n  output: llm/src/a/\n",
    )
    .unwrap();
    fs::write(
        stage_dir.join("stage_2.md"),
        "# Stage 2: Features\n\n## Tasks\n\nTASK-002 Handler\n  output: llm/src/b/\n",
    )
    .unwrap();

    // Sync should discover stages AND tasks
    hlv::cmd::task::run_sync(root, false).unwrap();

    let map = load_milestones(root);
    let current = map.current.as_ref().unwrap();
    assert_eq!(
        current.stages.len(),
        2,
        "Should have created 2 stage entries"
    );
    assert_eq!(current.stages[0].id, 1);
    assert_eq!(current.stages[0].scope, "Foundation");
    assert_eq!(current.stages[0].tasks.len(), 1);
    assert_eq!(current.stages[0].tasks[0].id, "TASK-001");
    assert_eq!(current.stages[1].id, 2);
    assert_eq!(current.stages[1].tasks[0].id, "TASK-002");
}

// ═══════════════════════════════════════════════════════
// Issue #4 — Missing test coverage
// ═══════════════════════════════════════════════════════

#[test]
fn sync_removes_pending_tasks() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let milestone_id = load_milestones(root).current.unwrap().id;
    let stage_dir = root.join("human/milestones").join(&milestone_id);

    // Start with 2 tasks in tracker
    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Test".to_string(),
        status: StageStatus::Pending,
        commit: None,
        tasks: vec![
            TaskTracker::new("TASK-001".to_string()),
            TaskTracker::new("TASK-002".to_string()),
        ],
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    // Stage plan only has TASK-001 now (TASK-002 removed)
    fs::write(
        stage_dir.join("stage_1.md"),
        "# Stage 1: Test\n\n## Tasks\n\nTASK-001 Only\n  output: llm/src/a/\n",
    )
    .unwrap();

    hlv::cmd::task::run_sync(root, false).unwrap();

    let map = load_milestones(root);
    let tasks = &map.current.as_ref().unwrap().stages[0].tasks;
    assert_eq!(tasks.len(), 1, "Pending task should be removed");
    assert_eq!(tasks[0].id, "TASK-001");
}

#[test]
fn sync_conflict_active_task_removal() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let milestone_id = load_milestones(root).current.unwrap().id;
    let stage_dir = root.join("human/milestones").join(&milestone_id);

    // TASK-002 is InProgress — can't be removed without --force
    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Test".to_string(),
        status: StageStatus::Implementing,
        commit: None,
        tasks: vec![TaskTracker::new("TASK-001".to_string()), {
            let mut t = TaskTracker::new("TASK-002".to_string());
            t.status = TaskStatus::InProgress;
            t.started_at = Some("2026-03-08T10:00:00Z".to_string());
            t
        }],
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    // Stage plan only has TASK-001 (TASK-002 removed from plan)
    fs::write(
        stage_dir.join("stage_1.md"),
        "# Stage 1: Test\n\n## Tasks\n\nTASK-001 Only\n  output: llm/src/a/\n",
    )
    .unwrap();

    // Should fail without --force
    let result = hlv::cmd::task::run_sync(root, false);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Cannot remove active tasks"),
        "Should mention active tasks"
    );

    // With --force should succeed
    hlv::cmd::task::run_sync(root, true).unwrap();
    let map = load_milestones(root);
    assert_eq!(map.current.as_ref().unwrap().stages[0].tasks.len(), 1);
}

#[test]
fn tsk040_negative_dep_not_done() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let milestone_id = load_milestones(root).current.unwrap().id;
    let stage_dir = root.join("human/milestones").join(&milestone_id);

    fs::write(
        stage_dir.join("stage_1.md"),
        "# Stage 1: Test\n\n## Tasks\n\nTASK-001 First\n  output: llm/src/a/\n\nTASK-002 Second\n  depends_on: [TASK-001]\n  output: llm/src/b/\n",
    )
    .unwrap();

    // TASK-002 is InProgress but TASK-001 is still Pending → TSK-040
    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Test".to_string(),
        status: StageStatus::Implementing,
        commit: None,
        tasks: vec![
            TaskTracker::new("TASK-001".to_string()), // Pending
            {
                let mut t = TaskTracker::new("TASK-002".to_string());
                t.status = TaskStatus::InProgress;
                t.started_at = Some("2026-03-08T10:00:00Z".to_string());
                t
            },
        ],
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    let (diags, _) = hlv::cmd::check::get_check_diagnostics(root).unwrap();
    assert!(
        diags
            .iter()
            .any(|d| d.code == "TSK-040" && d.message.contains("TASK-002")),
        "Expected TSK-040 for TASK-002 (dep TASK-001 not done), got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.starts_with("TSK"))
            .collect::<Vec<_>>()
    );
}
