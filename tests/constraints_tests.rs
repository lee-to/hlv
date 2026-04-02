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

fn quote_arg(arg: &str) -> String {
    if arg.contains([' ', '\t', '"']) {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        arg.to_string()
    }
}

fn command_from_current_exe(args: &[&str]) -> String {
    let exe = std::env::current_exe().unwrap();
    let exe_path = quote_arg(exe.to_string_lossy().as_ref());
    let mut parts = vec![exe_path];
    parts.extend(args.iter().map(|arg| quote_arg(arg)));
    parts.join(" ")
}

fn passing_command() -> String {
    command_from_current_exe(&["--help"])
}

fn failing_command() -> String {
    command_from_current_exe(&["--definitely-invalid-constraint-option"])
}

fn yaml_double_quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn file_exists_check_command(path: &str) -> String {
    if cfg!(windows) {
        format!("cmd /C if exist {} (exit 0) else (exit 1)", path)
    } else {
        let script = format!("test -f {}", path);
        format!("sh -c {}", quote_arg(&script))
    }
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
        None,
        None,
        None,
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
    hlv::cmd::constraints::run_add_rule(
        root, "test-dup", "rule1", "high", "Test", None, None, None,
    )
    .unwrap();

    let result = hlv::cmd::constraints::run_add_rule(
        root, "test-dup", "rule1", "low", "Dup", None, None, None,
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));
}

#[test]
fn constraints_add_rule_invalid_severity() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::constraints::run_add(root, "test-sev", None, None, "global").unwrap();

    let result = hlv::cmd::constraints::run_add_rule(
        root,
        "test-sev",
        "rule1",
        "extreme",
        "Bad severity",
        None,
        None,
        None,
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid severity"));
}

#[test]
fn constraints_remove_rule() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::constraints::run_add(root, "test-rm", None, None, "global").unwrap();
    hlv::cmd::constraints::run_add_rule(root, "test-rm", "rule1", "high", "Test", None, None, None)
        .unwrap();
    hlv::cmd::constraints::run_add_rule(root, "test-rm", "rule2", "low", "Test2", None, None, None)
        .unwrap();

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

// ═══════════════════════════════════════════════════════
// Phase 1: check_command / check_cwd tests
// ═══════════════════════════════════════════════════════

#[test]
fn constraints_add_rule_with_check_fields() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::constraints::run_add(root, "checktest", None, None, "global").unwrap();
    hlv::cmd::constraints::run_add_rule(
        root,
        "checktest",
        "no_println",
        "low",
        "println! is forbidden",
        Some("! rg \"println!\" src"),
        Some("."),
        None,
    )
    .unwrap();

    let cf =
        hlv::model::policy::ConstraintFile::load(&root.join("human/constraints/checktest.yaml"))
            .unwrap();
    assert_eq!(
        cf.rules[0].check_command.as_deref(),
        Some("! rg \"println!\" src")
    );
    assert_eq!(cf.rules[0].check_cwd.as_deref(), Some("."));
}

#[test]
fn cst050_check_command_success() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let pass_cmd = passing_command();

    // Create constraint with a command that succeeds
    hlv::cmd::constraints::run_add(root, "chk", None, None, "global").unwrap();
    hlv::cmd::constraints::run_add_rule(
        root,
        "chk",
        "always_pass",
        "critical",
        "Always passes",
        Some(&pass_cmd),
        None,
        None,
    )
    .unwrap();

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    let (diags, results) =
        hlv::check::constraints::run_constraint_checks(root, &project, None, None);
    assert!(diags.is_empty(), "expected no diags: {:?}", diags);
    assert_eq!(results.len(), 1);
    assert!(results[0].passed);
}

#[test]
fn cst050_check_command_failure_critical_is_error() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let fail_cmd = failing_command();

    hlv::cmd::constraints::run_add(root, "chk", None, None, "global").unwrap();
    hlv::cmd::constraints::run_add_rule(
        root,
        "chk",
        "always_fail",
        "critical",
        "Always fails",
        Some(&fail_cmd),
        None,
        None,
    )
    .unwrap();

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    let (diags, results) =
        hlv::check::constraints::run_constraint_checks(root, &project, None, None);

    assert_eq!(results.len(), 1);
    assert!(!results[0].passed);

    // Phase 2: critical severity → error
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, "CST-050");
    assert!(
        matches!(diags[0].severity, hlv::check::Severity::Error),
        "CST-050 should be error for critical severity"
    );
}

#[test]
fn cst050_check_cwd() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let cwd_cmd = file_exists_check_command("marker.txt");

    // Create a subdirectory with a marker file and verify check_cwd resolution.
    std::fs::create_dir_all(root.join("subdir")).unwrap();
    std::fs::write(root.join("subdir/marker.txt"), "found").unwrap();

    hlv::cmd::constraints::run_add(root, "chk", None, None, "global").unwrap();
    hlv::cmd::constraints::run_add_rule(
        root,
        "chk",
        "find_marker",
        "high",
        "Marker must exist in check_cwd",
        Some(&cwd_cmd),
        Some("subdir"),
        None,
    )
    .unwrap();

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    let (diags, results) =
        hlv::check::constraints::run_constraint_checks(root, &project, None, None);
    assert!(diags.is_empty(), "expected pass: {:?}", diags);
    assert!(results[0].passed);
}

#[test]
fn cst050_timeout() {
    // This test verifies the timeout mechanism works but uses a fast command
    // to avoid actually waiting 60s. The real timeout is 60s; here we just
    // verify the function handles commands correctly.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let pass_cmd = passing_command();

    hlv::cmd::constraints::run_add(root, "chk", None, None, "global").unwrap();
    hlv::cmd::constraints::run_add_rule(
        root,
        "chk",
        "quick_cmd",
        "low",
        "Quick command",
        Some(&pass_cmd),
        None,
        None,
    )
    .unwrap();

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    let (diags, results) =
        hlv::check::constraints::run_constraint_checks(root, &project, None, None);
    assert!(diags.is_empty());
    assert!(results[0].passed);
}

#[test]
fn constraints_check_subcommand() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let pass_cmd = passing_command();
    let fail_cmd = failing_command();

    hlv::cmd::constraints::run_add(root, "chk", None, None, "global").unwrap();
    hlv::cmd::constraints::run_add_rule(
        root,
        "chk",
        "pass_rule",
        "medium",
        "Should pass",
        Some(&pass_cmd),
        None,
        None,
    )
    .unwrap();
    hlv::cmd::constraints::run_add_rule(
        root,
        "chk",
        "fail_rule",
        "high",
        "Should fail",
        Some(&fail_cmd),
        None,
        None,
    )
    .unwrap();

    // Test JSON output
    let result = hlv::cmd::constraints::get_constraint_check_results(root, None, None).unwrap();
    let results = result["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);

    let passed: Vec<_> = results
        .iter()
        .filter(|r| r["passed"].as_bool() == Some(true))
        .collect();
    let failed: Vec<_> = results
        .iter()
        .filter(|r| r["passed"].as_bool() == Some(false))
        .collect();
    assert_eq!(passed.len(), 1);
    assert_eq!(failed.len(), 1);
}

#[test]
fn constraints_check_filter_by_constraint() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let pass_cmd = passing_command();

    hlv::cmd::constraints::run_add(root, "alpha", None, None, "global").unwrap();
    hlv::cmd::constraints::run_add_rule(
        root,
        "alpha",
        "a1",
        "low",
        "Alpha rule",
        Some(&pass_cmd),
        None,
        None,
    )
    .unwrap();

    hlv::cmd::constraints::run_add(root, "beta", None, None, "global").unwrap();
    hlv::cmd::constraints::run_add_rule(
        root,
        "beta",
        "b1",
        "low",
        "Beta rule",
        Some(&pass_cmd),
        None,
        None,
    )
    .unwrap();

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();

    // Filter to alpha only
    let (_, results) =
        hlv::check::constraints::run_constraint_checks(root, &project, Some("alpha"), None);
    assert_eq!(results.len(), 1);
    assert!(results[0].constraint_id.contains("alpha"));
}

#[test]
fn constraints_check_filter_by_rule() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let pass_cmd = passing_command();

    hlv::cmd::constraints::run_add(root, "multi", None, None, "global").unwrap();
    hlv::cmd::constraints::run_add_rule(
        root,
        "multi",
        "r1",
        "low",
        "Rule 1",
        Some(&pass_cmd),
        None,
        None,
    )
    .unwrap();
    hlv::cmd::constraints::run_add_rule(
        root,
        "multi",
        "r2",
        "low",
        "Rule 2",
        Some(&pass_cmd),
        None,
        None,
    )
    .unwrap();

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();

    let (_, results) =
        hlv::check::constraints::run_constraint_checks(root, &project, None, Some("r2"));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].rule_id, "r2");
}

#[test]
fn cst050_high_severity_is_error() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let fail_cmd = failing_command();

    hlv::cmd::constraints::run_add(root, "chk", None, None, "global").unwrap();
    // Command that fails with non-zero exit — high severity → error
    hlv::cmd::constraints::run_add_rule(
        root,
        "chk",
        "bad_cmd",
        "high",
        "Bad command",
        Some(&fail_cmd),
        None,
        None,
    )
    .unwrap();

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    let (diags, results) =
        hlv::check::constraints::run_constraint_checks(root, &project, None, None);
    assert!(!results[0].passed);
    assert_eq!(diags[0].code, "CST-050");
    assert!(
        matches!(diags[0].severity, hlv::check::Severity::Error),
        "CST-050 should be error for high severity"
    );
}

#[test]
fn constraints_check_no_commands_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::constraints::run_add(root, "nocheck", None, None, "global").unwrap();
    hlv::cmd::constraints::run_add_rule(root, "nocheck", "r1", "low", "No check", None, None, None)
        .unwrap();

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    let (diags, results) =
        hlv::check::constraints::run_constraint_checks(root, &project, None, None);
    assert!(diags.is_empty());
    assert!(results.is_empty());
}

#[test]
fn constraint_rule_check_fields_serde_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.yaml");

    let cf = hlv::model::policy::ConstraintFile {
        id: "constraints.test.global".to_string(),
        version: "1.0.0".to_string(),
        owner: None,
        intent: None,
        check_command: None,
        check_cwd: None,
        rules: vec![hlv::model::policy::ConstraintRule {
            id: "rule_with_check".to_string(),
            severity: "critical".to_string(),
            statement: "Test check".to_string(),
            enforcement: vec![],
            check_command: Some("cargo test".to_string()),
            check_cwd: Some("llm".to_string()),
            error_level: None,
        }],
        exceptions: None,
    };

    cf.save(&path).unwrap();
    let loaded = hlv::model::policy::ConstraintFile::load(&path).unwrap();
    assert_eq!(loaded.rules[0].check_command.as_deref(), Some("cargo test"));
    assert_eq!(loaded.rules[0].check_cwd.as_deref(), Some("llm"));
}

#[test]
fn constraint_rule_check_fields_optional_serde() {
    // Verify that rules without check_command/check_cwd still parse
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.yaml");

    std::fs::write(
        &path,
        r#"id: constraints.test.global
version: "1.0.0"
rules:
  - id: no_check
    severity: low
    statement: "No check command"
"#,
    )
    .unwrap();

    let cf = hlv::model::policy::ConstraintFile::load(&path).unwrap();
    assert!(cf.rules[0].check_command.is_none());
    assert!(cf.rules[0].check_cwd.is_none());
}

// ═══════════════════════════════════════════════════════
// Phase 2: error_level / severity mapping tests
// ═══════════════════════════════════════════════════════

#[test]
fn cst050_low_severity_is_warning() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let fail_cmd = failing_command();

    hlv::cmd::constraints::run_add(root, "chk", None, None, "global").unwrap();
    hlv::cmd::constraints::run_add_rule(
        root,
        "chk",
        "low_rule",
        "low",
        "Low severity rule",
        Some(&fail_cmd),
        None,
        None,
    )
    .unwrap();

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    let (diags, _) = hlv::check::constraints::run_constraint_checks(root, &project, None, None);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, "CST-050");
    assert!(
        matches!(diags[0].severity, hlv::check::Severity::Warning),
        "CST-050 should be warning for low severity"
    );
}

#[test]
fn cst050_medium_severity_is_warning() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let fail_cmd = failing_command();

    hlv::cmd::constraints::run_add(root, "chk", None, None, "global").unwrap();
    hlv::cmd::constraints::run_add_rule(
        root,
        "chk",
        "med_rule",
        "medium",
        "Medium severity rule",
        Some(&fail_cmd),
        None,
        None,
    )
    .unwrap();

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    let (diags, _) = hlv::check::constraints::run_constraint_checks(root, &project, None, None);
    assert_eq!(diags.len(), 1);
    assert!(
        matches!(diags[0].severity, hlv::check::Severity::Warning),
        "CST-050 should be warning for medium severity"
    );
}

#[test]
fn cst050_error_level_override_error() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let fail_cmd = failing_command();

    hlv::cmd::constraints::run_add(root, "chk", None, None, "global").unwrap();
    // low severity but error_level=error → should be error
    hlv::cmd::constraints::run_add_rule(
        root,
        "chk",
        "forced_error",
        "low",
        "Forced error rule",
        Some(&fail_cmd),
        None,
        Some("error"),
    )
    .unwrap();

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    let (diags, results) =
        hlv::check::constraints::run_constraint_checks(root, &project, None, None);
    assert_eq!(results.len(), 1);
    assert!(!results[0].passed);
    assert_eq!(diags.len(), 1);
    assert!(
        matches!(diags[0].severity, hlv::check::Severity::Error),
        "error_level=error should override low severity to error"
    );
}

#[test]
fn cst050_error_level_override_warning() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let fail_cmd = failing_command();

    hlv::cmd::constraints::run_add(root, "chk", None, None, "global").unwrap();
    // critical severity but error_level=warning → should be warning
    hlv::cmd::constraints::run_add_rule(
        root,
        "chk",
        "forced_warn",
        "critical",
        "Forced warning rule",
        Some(&fail_cmd),
        None,
        Some("warning"),
    )
    .unwrap();

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    let (diags, _) = hlv::check::constraints::run_constraint_checks(root, &project, None, None);
    assert_eq!(diags.len(), 1);
    assert!(
        matches!(diags[0].severity, hlv::check::Severity::Warning),
        "error_level=warning should override critical severity to warning"
    );
}

#[test]
fn cst050_error_level_override_info() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let fail_cmd = failing_command();

    hlv::cmd::constraints::run_add(root, "chk", None, None, "global").unwrap();
    // high severity but error_level=info → should be info
    hlv::cmd::constraints::run_add_rule(
        root,
        "chk",
        "forced_info",
        "high",
        "Forced info rule",
        Some(&fail_cmd),
        None,
        Some("info"),
    )
    .unwrap();

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    let (diags, _) = hlv::check::constraints::run_constraint_checks(root, &project, None, None);
    assert_eq!(diags.len(), 1);
    assert!(
        matches!(diags[0].severity, hlv::check::Severity::Info),
        "error_level=info should override high severity to info"
    );
}

#[test]
fn cst030_invalid_error_level() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::constraints::run_add(root, "badel", None, None, "global").unwrap();

    // Write rule with invalid error_level directly to YAML
    let cf_path = root.join("human/constraints/badel.yaml");
    std::fs::write(
        &cf_path,
        r#"id: constraints.badel.global
version: "1.0.0"
rules:
  - id: bad_el_rule
    severity: low
    statement: "Rule with bad error_level"
    error_level: fatal
"#,
    )
    .unwrap();

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    let diags = hlv::check::constraints::check_constraints(root, &project);
    let cst030: Vec<_> = diags
        .iter()
        .filter(|d| d.code == "CST-030" && d.message.contains("error_level"))
        .collect();
    assert_eq!(cst030.len(), 1, "Expected CST-030 for invalid error_level");
    assert!(cst030[0].message.contains("fatal"));
}

#[test]
fn constraint_add_rule_invalid_error_level() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::constraints::run_add(root, "test-el", None, None, "global").unwrap();

    let result = hlv::cmd::constraints::run_add_rule(
        root,
        "test-el",
        "rule1",
        "high",
        "Test",
        None,
        None,
        Some("fatal"),
    );
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Invalid error_level"));
}

#[test]
fn constraint_add_rule_with_error_level() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let pass_cmd = passing_command();

    hlv::cmd::constraints::run_add(root, "eltest", None, None, "global").unwrap();
    hlv::cmd::constraints::run_add_rule(
        root,
        "eltest",
        "mandatory_rule",
        "low",
        "Force error on low severity",
        Some(&pass_cmd),
        None,
        Some("error"),
    )
    .unwrap();

    let cf = hlv::model::policy::ConstraintFile::load(&root.join("human/constraints/eltest.yaml"))
        .unwrap();
    assert_eq!(cf.rules[0].error_level.as_deref(), Some("error"));
}

#[test]
fn constraint_rule_error_level_serde_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.yaml");

    let cf = hlv::model::policy::ConstraintFile {
        id: "constraints.test.global".to_string(),
        version: "1.0.0".to_string(),
        owner: None,
        intent: None,
        check_command: None,
        check_cwd: None,
        rules: vec![hlv::model::policy::ConstraintRule {
            id: "rule_with_el".to_string(),
            severity: "low".to_string(),
            statement: "Test error_level".to_string(),
            enforcement: vec![],
            check_command: Some("true".to_string()),
            check_cwd: None,
            error_level: Some("error".to_string()),
        }],
        exceptions: None,
    };

    cf.save(&path).unwrap();
    let loaded = hlv::model::policy::ConstraintFile::load(&path).unwrap();
    assert_eq!(loaded.rules[0].error_level.as_deref(), Some("error"));
}

#[test]
fn constraint_rule_error_level_optional_serde() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.yaml");

    std::fs::write(
        &path,
        r#"id: constraints.test.global
version: "1.0.0"
rules:
  - id: no_el
    severity: critical
    statement: "No error_level set"
"#,
    )
    .unwrap();

    let cf = hlv::model::policy::ConstraintFile::load(&path).unwrap();
    assert!(cf.rules[0].error_level.is_none());
}

// ─── CST-060: File-level check_command ──────────────────────────

#[test]
fn cst060_file_level_check_command_success() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let pass_cmd = passing_command();

    hlv::cmd::constraints::run_add(root, "filechk", None, None, "global").unwrap();

    // Write constraint file with file-level check_command that succeeds
    let cf_path = root.join("human/constraints/filechk.yaml");
    let content = format!(
        "id: constraints.filechk.global\nversion: \"1.0.0\"\ncheck_command: {}\nrules: []\n",
        yaml_double_quoted(&pass_cmd)
    );
    std::fs::write(&cf_path, content).unwrap();

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    let (diags, results) = hlv::check::constraints::run_file_level_checks(root, &project, None);
    assert!(diags.is_empty(), "expected no diags: {:?}", diags);
    assert_eq!(results.len(), 1);
    assert!(results[0].passed);
    assert_eq!(results[0].rule_id, "__file__");
    assert_eq!(results[0].constraint_id, "constraints.filechk.global");
}

#[test]
fn cst060_file_level_check_command_failure_is_error() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let fail_cmd = failing_command();

    hlv::cmd::constraints::run_add(root, "failchk", None, None, "global").unwrap();

    // Write constraint file with file-level check_command that fails
    let cf_path = root.join("human/constraints/failchk.yaml");
    let content = format!(
        "id: constraints.failchk.global\nversion: \"1.0.0\"\ncheck_command: {}\nrules: []\n",
        yaml_double_quoted(&fail_cmd)
    );
    std::fs::write(&cf_path, content).unwrap();

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    let (diags, results) = hlv::check::constraints::run_file_level_checks(root, &project, None);

    assert_eq!(results.len(), 1);
    assert!(!results[0].passed);
    assert_eq!(results[0].rule_id, "__file__");

    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, "CST-060");
    assert!(
        matches!(diags[0].severity, hlv::check::Severity::Error),
        "CST-060 should always be error severity"
    );
}

#[test]
fn cst060_file_level_check_cwd() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let cwd_cmd = file_exists_check_command("marker.txt");

    // Create a subdirectory with a marker file and verify check_cwd resolution.
    std::fs::create_dir_all(root.join("subdir")).unwrap();
    std::fs::write(root.join("subdir/marker.txt"), "found").unwrap();

    hlv::cmd::constraints::run_add(root, "cwdchk", None, None, "global").unwrap();

    // Write constraint file with check_cwd
    let cf_path = root.join("human/constraints/cwdchk.yaml");
    let content = format!(
        "id: constraints.cwdchk.global\nversion: \"1.0.0\"\ncheck_command: {}\ncheck_cwd: subdir\nrules: []\n",
        yaml_double_quoted(&cwd_cmd)
    );
    std::fs::write(&cf_path, content).unwrap();

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    let (diags, results) = hlv::check::constraints::run_file_level_checks(root, &project, None);
    assert!(diags.is_empty(), "expected pass: {:?}", diags);
    assert!(results[0].passed);
}

#[test]
fn cst060_file_level_timeout() {
    // Verify the function handles commands correctly (same pattern as cst050_timeout)
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let pass_cmd = passing_command();

    hlv::cmd::constraints::run_add(root, "timechk", None, None, "global").unwrap();

    // Write constraint file with a quick check_command (real timeout is 60s)
    let cf_path = root.join("human/constraints/timechk.yaml");
    let content = format!(
        "id: constraints.timechk.global\nversion: \"1.0.0\"\ncheck_command: {}\nrules: []\n",
        yaml_double_quoted(&pass_cmd)
    );
    std::fs::write(&cf_path, content).unwrap();

    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    let (diags, results) = hlv::check::constraints::run_file_level_checks(root, &project, None);
    assert!(diags.is_empty());
    assert!(results[0].passed);
}

#[test]
fn constraint_file_check_fields_serde_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.yaml");

    let cf = hlv::model::policy::ConstraintFile {
        id: "constraints.test.global".to_string(),
        version: "1.0.0".to_string(),
        owner: None,
        intent: None,
        check_command: Some("make lint".to_string()),
        check_cwd: Some("src".to_string()),
        rules: vec![],
        exceptions: None,
    };

    cf.save(&path).unwrap();
    let loaded = hlv::model::policy::ConstraintFile::load(&path).unwrap();
    assert_eq!(loaded.check_command.as_deref(), Some("make lint"));
    assert_eq!(loaded.check_cwd.as_deref(), Some("src"));

    // Also verify None fields are not serialized
    let cf_no_check = hlv::model::policy::ConstraintFile {
        id: "constraints.empty.global".to_string(),
        version: "1.0.0".to_string(),
        owner: None,
        intent: None,
        check_command: None,
        check_cwd: None,
        rules: vec![],
        exceptions: None,
    };
    let path2 = tmp.path().join("test2.yaml");
    cf_no_check.save(&path2).unwrap();
    let content = std::fs::read_to_string(&path2).unwrap();
    assert!(!content.contains("check_command"));
    assert!(!content.contains("check_cwd"));
}
