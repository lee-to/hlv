use tempfile::TempDir;

fn setup_project(root: &std::path::Path) {
    hlv::cmd::init::run_with_milestone(
        root.to_str().unwrap(),
        Some("test-project"),
        Some("qa"),
        Some("claude"),
        Some("init"),
        Some("minimal"),
    )
    .unwrap();
}

#[test]
fn gates_add_creates_gate() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::gates::run_add(
        root,
        "GATE-LINT-001",
        "lint",
        true,
        Some("cargo clippy"),
        Some("llm"),
        true,
    )
    .unwrap();

    let policy =
        hlv::model::policy::GatesPolicy::load(&root.join("validation/gates-policy.yaml")).unwrap();
    let gate = policy
        .gates
        .iter()
        .find(|g| g.id == "GATE-LINT-001")
        .unwrap();
    assert_eq!(gate.gate_type, "lint");
    assert!(gate.mandatory);
    assert!(gate.enabled);
    assert_eq!(gate.command.as_deref(), Some("cargo clippy"));
    assert_eq!(gate.cwd.as_deref(), Some("llm"));
}

#[test]
fn gates_add_duplicate_fails() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::gates::run_add(root, "GATE-LINT-001", "lint", false, None, None, true).unwrap();

    let result = hlv::cmd::gates::run_add(root, "GATE-LINT-001", "lint", false, None, None, true);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));
}

#[test]
fn gates_add_disabled() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::gates::run_add(root, "GATE-TEST-001", "custom", false, None, None, false).unwrap();

    let policy =
        hlv::model::policy::GatesPolicy::load(&root.join("validation/gates-policy.yaml")).unwrap();
    let gate = policy
        .gates
        .iter()
        .find(|g| g.id == "GATE-TEST-001")
        .unwrap();
    assert!(!gate.enabled);
}

#[test]
fn gates_remove_with_force() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::gates::run_add(root, "GATE-RM-001", "lint", true, None, None, true).unwrap();

    hlv::cmd::gates::run_remove(root, "GATE-RM-001", true).unwrap();

    let policy =
        hlv::model::policy::GatesPolicy::load(&root.join("validation/gates-policy.yaml")).unwrap();
    assert!(policy.gates.iter().all(|g| g.id != "GATE-RM-001"));
}

#[test]
fn gates_remove_not_found() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::cmd::gates::run_remove(root, "GATE-NONEXISTENT", true);
    assert!(result.is_err());
}

#[test]
fn gates_edit_type_and_mandatory() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::gates::run_add(root, "GATE-EDIT-001", "lint", false, None, None, true).unwrap();

    // Change type and set mandatory
    hlv::cmd::gates::run_edit(root, "GATE-EDIT-001", Some("security"), true, false).unwrap();

    let policy =
        hlv::model::policy::GatesPolicy::load(&root.join("validation/gates-policy.yaml")).unwrap();
    let gate = policy
        .gates
        .iter()
        .find(|g| g.id == "GATE-EDIT-001")
        .unwrap();
    assert_eq!(gate.gate_type, "security");
    assert!(gate.mandatory);

    // Clear mandatory
    hlv::cmd::gates::run_edit(root, "GATE-EDIT-001", None, false, true).unwrap();

    let policy =
        hlv::model::policy::GatesPolicy::load(&root.join("validation/gates-policy.yaml")).unwrap();
    let gate = policy
        .gates
        .iter()
        .find(|g| g.id == "GATE-EDIT-001")
        .unwrap();
    assert!(!gate.mandatory);
}

#[test]
fn gates_run_single_gate() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::gates::run_add(root, "GATE-A", "lint", false, Some("true"), None, true).unwrap();
    hlv::cmd::gates::run_add(root, "GATE-B", "lint", false, Some("false"), None, true).unwrap();

    // Run only GATE-A (should pass)
    let (passed, failed, _) = hlv::cmd::gates::run_gate_commands(root, Some("GATE-A")).unwrap();
    assert_eq!(passed, 1);
    assert_eq!(failed, 0);

    // Run only GATE-B (should fail)
    let (passed, failed, _) = hlv::cmd::gates::run_gate_commands(root, Some("GATE-B")).unwrap();
    assert_eq!(passed, 0);
    assert_eq!(failed, 1);
}

#[test]
fn gates_json_output() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // Just verify it doesn't panic
    hlv::cmd::gates::run_show_json(root).unwrap();
}
