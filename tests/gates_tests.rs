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
fn gates_set_command_rejects_invalid_command_and_does_not_save() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result =
        hlv::cmd::gates::run_set_command(root, "GATE-CONTRACT-001", "cargo test && cargo clippy");

    let err = result.unwrap_err().to_string();
    assert!(err.contains("unsupported command syntax '&&'"), "{err}");

    let policy =
        hlv::model::policy::GatesPolicy::load(&root.join("validation/gates-policy.yaml")).unwrap();
    let gate = policy
        .gates
        .iter()
        .find(|g| g.id == "GATE-CONTRACT-001")
        .unwrap();
    assert!(gate.command.is_none());
}

#[test]
fn gates_add_rejects_invalid_command() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::cmd::gates::run_add(
        root,
        "GATE-BAD-CMD",
        "custom",
        false,
        Some("echo ok && echo bad"),
        None,
        true,
    );

    let err = result.unwrap_err().to_string();
    assert!(err.contains("unsupported command syntax '&&'"), "{err}");

    let policy =
        hlv::model::policy::GatesPolicy::load(&root.join("validation/gates-policy.yaml")).unwrap();
    assert!(policy.gates.iter().all(|g| g.id != "GATE-BAD-CMD"));
}

#[test]
fn gates_set_cwd_rejects_missing_directory_and_does_not_save() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::cmd::gates::run_set_cwd(root, "GATE-CONTRACT-001", "apps/backend");

    let err = result.unwrap_err().to_string();
    assert_eq!(
        err,
        "Gate 'GATE-CONTRACT-001' cwd does not exist: apps/backend"
    );

    let policy =
        hlv::model::policy::GatesPolicy::load(&root.join("validation/gates-policy.yaml")).unwrap();
    let gate = policy
        .gates
        .iter()
        .find(|g| g.id == "GATE-CONTRACT-001")
        .unwrap();
    assert!(gate.cwd.is_none());
}

#[test]
fn gates_set_cwd_rejects_absolute_directory_and_does_not_save() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let absolute_cwd = root.to_string_lossy();

    let result = hlv::cmd::gates::run_set_cwd(root, "GATE-CONTRACT-001", &absolute_cwd);

    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Gate 'GATE-CONTRACT-001' cwd must be relative to project root"),
        "{err}"
    );

    let policy =
        hlv::model::policy::GatesPolicy::load(&root.join("validation/gates-policy.yaml")).unwrap();
    let gate = policy
        .gates
        .iter()
        .find(|g| g.id == "GATE-CONTRACT-001")
        .unwrap();
    assert!(gate.cwd.is_none());
}

#[test]
fn gates_add_rejects_missing_cwd() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::cmd::gates::run_add(
        root,
        "GATE-BAD-CWD",
        "custom",
        false,
        None,
        Some("apps/backend"),
        true,
    );

    let err = result.unwrap_err().to_string();
    assert_eq!(err, "Gate 'GATE-BAD-CWD' cwd does not exist: apps/backend");

    let policy =
        hlv::model::policy::GatesPolicy::load(&root.join("validation/gates-policy.yaml")).unwrap();
    assert!(policy.gates.iter().all(|g| g.id != "GATE-BAD-CWD"));
}

#[test]
fn gates_add_rejects_traversal_cwd() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::cmd::gates::run_add(
        root,
        "GATE-BAD-CWD",
        "custom",
        false,
        None,
        Some("llm/.."),
        true,
    );

    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Gate 'GATE-BAD-CWD' cwd must be relative to project root"),
        "{err}"
    );

    let policy =
        hlv::model::policy::GatesPolicy::load(&root.join("validation/gates-policy.yaml")).unwrap();
    assert!(policy.gates.iter().all(|g| g.id != "GATE-BAD-CWD"));
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

    let pass_cmd = command_from_current_exe(&["--help"]);
    let fail_cmd = command_from_current_exe(&["--definitely-invalid-gate-option"]);

    hlv::cmd::gates::run_add(root, "GATE-A", "lint", false, Some(&pass_cmd), None, true).unwrap();
    hlv::cmd::gates::run_add(root, "GATE-B", "lint", false, Some(&fail_cmd), None, true).unwrap();

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
fn gates_run_missing_program_fails() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::gates::run_add(
        root,
        "GATE-MISSING",
        "lint",
        false,
        Some("definitely-not-a-real-hlv-gate-binary"),
        None,
        true,
    )
    .unwrap();

    let (passed, failed, _) =
        hlv::cmd::gates::run_gate_commands(root, Some("GATE-MISSING")).unwrap();
    assert_eq!(passed, 0);
    assert_eq!(failed, 1);
}

#[test]
fn gates_run_missing_cwd_reports_structured_failure() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let pass_cmd = command_from_current_exe(&["--help"]);
    hlv::cmd::gates::run_add(
        root,
        "GATE-CWD-MISSING",
        "custom",
        false,
        Some(&pass_cmd),
        None,
        true,
    )
    .unwrap();

    let policy_path = root.join("validation/gates-policy.yaml");
    let mut policy = hlv::model::policy::GatesPolicy::load(&policy_path).unwrap();
    policy.find_gate_mut("GATE-CWD-MISSING").unwrap().cwd = Some("apps/backend".to_string());
    policy.save(&policy_path).unwrap();

    let summary =
        hlv::cmd::gates::run_gate_commands_with_results(root, Some("GATE-CWD-MISSING"), false)
            .unwrap();

    assert_eq!((summary.passed, summary.failed, summary.skipped), (0, 1, 0));
    assert_eq!(summary.results.len(), 1);
    let result = &summary.results[0];
    assert_eq!(result.id, "GATE-CWD-MISSING");
    assert_eq!(result.status, hlv::model::milestone::GateRunStatus::Failed);
    assert_eq!(
        result.reason,
        "Gate 'GATE-CWD-MISSING' cwd does not exist: apps/backend"
    );
    assert_eq!(result.cwd, "apps/backend");
    assert_eq!(result.command.as_deref(), Some(pass_cmd.as_str()));
}

#[test]
fn gates_run_structured_results_include_skipped_gates() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let no_command =
        hlv::cmd::gates::run_gate_commands_with_results(root, Some("GATE-CONTRACT-001"), false)
            .unwrap();
    assert_eq!(
        (no_command.passed, no_command.failed, no_command.skipped),
        (0, 0, 1)
    );
    assert_eq!(no_command.results.len(), 1);
    assert_eq!(
        no_command.results[0].status,
        hlv::model::milestone::GateRunStatus::Skipped
    );
    assert_eq!(no_command.results[0].reason, "no command");
    assert!(no_command.results[0].command.is_none());

    hlv::cmd::gates::run_add(root, "GATE-DISABLED", "custom", false, None, None, false).unwrap();
    let disabled =
        hlv::cmd::gates::run_gate_commands_with_results(root, Some("GATE-DISABLED"), false)
            .unwrap();
    assert_eq!(
        (disabled.passed, disabled.failed, disabled.skipped),
        (0, 0, 1)
    );
    assert_eq!(
        disabled.results[0].status,
        hlv::model::milestone::GateRunStatus::Skipped
    );
    assert_eq!(disabled.results[0].reason, "disabled");
    assert_eq!(disabled.results[0].cwd, ".");
}

#[test]
fn gates_run_supports_quoted_argument_with_spaces() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let quoted_arg_cmd =
        command_from_current_exe(&["--exact", "this argument has spaces and matches nothing"]);
    hlv::cmd::gates::run_add(
        root,
        "GATE-QUOTED",
        "custom",
        false,
        Some(&quoted_arg_cmd),
        None,
        true,
    )
    .unwrap();

    let (passed, failed, _) =
        hlv::cmd::gates::run_gate_commands(root, Some("GATE-QUOTED")).unwrap();
    assert_eq!(passed, 1);
    assert_eq!(failed, 0);
}

#[test]
fn gates_run_rejects_shell_syntax() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::cmd::gates::run_add(root, "GATE-SHELL-SYNTAX", "custom", false, None, None, true).unwrap();

    let policy_path = root.join("validation/gates-policy.yaml");
    let mut policy = hlv::model::policy::GatesPolicy::load(&policy_path).unwrap();
    policy.find_gate_mut("GATE-SHELL-SYNTAX").unwrap().command =
        Some("echo ok && echo bad".to_string());
    policy.save(&policy_path).unwrap();

    let (passed, failed, _) =
        hlv::cmd::gates::run_gate_commands(root, Some("GATE-SHELL-SYNTAX")).unwrap();
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
