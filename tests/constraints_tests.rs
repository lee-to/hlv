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
fn constraints_list_shows_existing() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // Should not panic — init creates security + performance + observability
    hlv::cmd::constraints::run_list(root, None, false).unwrap();
}

#[test]
fn constraints_list_json() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::constraints::run_list(root, None, true).unwrap();
}

#[test]
fn constraints_show_security() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::constraints::run_show(root, "security", false).unwrap();
}

#[test]
fn constraints_show_json() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::constraints::run_show(root, "security", true).unwrap();
}

#[test]
fn constraints_add_creates_files() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::constraints::run_add(
        root,
        "compliance",
        Some("legal-team"),
        Some("SOC2 compliance"),
        "global",
    )
    .unwrap();

    // Constraint file should exist
    assert!(root.join("human/constraints/compliance.yaml").exists());

    // Should be in project.yaml
    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    assert!(project
        .constraints
        .iter()
        .any(|c| c.id == "constraints.compliance.global"));

    // Should be parseable
    let cf =
        hlv::model::policy::ConstraintFile::load(&root.join("human/constraints/compliance.yaml"))
            .unwrap();
    assert_eq!(cf.id, "constraints.compliance.global");
    assert_eq!(cf.owner.as_deref(), Some("legal-team"));
    assert_eq!(cf.intent.as_deref(), Some("SOC2 compliance"));
    assert!(cf.rules.is_empty());
}

#[test]
fn constraints_add_duplicate_fails() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // security already exists from init
    let result = hlv::cmd::constraints::run_add(root, "security", None, None, "global");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));
}

#[test]
fn constraints_remove_with_force() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::constraints::run_add(root, "test-cst", None, None, "global").unwrap();
    assert!(root.join("human/constraints/test-cst.yaml").exists());

    hlv::cmd::constraints::run_remove(root, "test-cst", true).unwrap();

    assert!(!root.join("human/constraints/test-cst.yaml").exists());

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    assert!(!project
        .constraints
        .iter()
        .any(|c| c.id.contains("test-cst")));
}

#[test]
fn constraints_remove_not_found() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::cmd::constraints::run_remove(root, "nonexistent", true);
    assert!(result.is_err());
}

#[test]
fn constraints_add_rule() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::constraints::run_add(root, "compliance", None, None, "global").unwrap();

    hlv::cmd::constraints::run_add_rule(
        root,
        "compliance",
        "audit_logs_required",
        "critical",
        "All state changes must produce audit logs",
    )
    .unwrap();

    let cf =
        hlv::model::policy::ConstraintFile::load(&root.join("human/constraints/compliance.yaml"))
            .unwrap();
    assert_eq!(cf.rules.len(), 1);
    assert_eq!(cf.rules[0].id, "audit_logs_required");
    assert_eq!(cf.rules[0].severity, "critical");
}

#[test]
fn constraints_add_rule_duplicate_fails() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::constraints::run_add(root, "test-dup", None, None, "global").unwrap();
    hlv::cmd::constraints::run_add_rule(root, "test-dup", "rule1", "high", "Test").unwrap();

    let result = hlv::cmd::constraints::run_add_rule(root, "test-dup", "rule1", "low", "Dup");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));
}

#[test]
fn constraints_add_rule_invalid_severity() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::constraints::run_add(root, "test-sev", None, None, "global").unwrap();

    let result =
        hlv::cmd::constraints::run_add_rule(root, "test-sev", "rule1", "extreme", "Bad severity");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid severity"));
}

#[test]
fn constraints_remove_rule() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::constraints::run_add(root, "test-rm", None, None, "global").unwrap();
    hlv::cmd::constraints::run_add_rule(root, "test-rm", "rule1", "high", "Test").unwrap();
    hlv::cmd::constraints::run_add_rule(root, "test-rm", "rule2", "low", "Test2").unwrap();

    hlv::cmd::constraints::run_remove_rule(root, "test-rm", "rule1").unwrap();

    let cf = hlv::model::policy::ConstraintFile::load(&root.join("human/constraints/test-rm.yaml"))
        .unwrap();
    assert_eq!(cf.rules.len(), 1);
    assert_eq!(cf.rules[0].id, "rule2");
}

#[test]
fn constraints_remove_rule_not_found() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::constraints::run_add(root, "test-nf", None, None, "global").unwrap();

    let result = hlv::cmd::constraints::run_remove_rule(root, "test-nf", "nonexistent");
    assert!(result.is_err());
}

#[test]
fn constraints_list_severity_filter() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // Just verify it doesn't panic with severity filter
    hlv::cmd::constraints::run_list(root, Some("critical"), false).unwrap();
}
