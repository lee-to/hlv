use std::fs;
use std::path::Path;

use chrono::NaiveDate;
use tempfile::TempDir;

use hlv::check::{Diagnostic, Severity};
use hlv::cmd::check::{get_check_report, CheckOptions};
use hlv::cmd::doctor::doctor_report;
use hlv::cmd::explain::lookup_diagnostic;
use hlv::cmd::waivers::audit_waivers;
use hlv::model::project::{ProjectMap, Strictness, VerifyStatus};
use hlv::model::waiver::WaiverFile;
use hlv::util::display_width::{display_width, pad_display_width, truncate_display_width};

fn write_minimal_project(root: &Path, strictness: Option<&str>) {
    fs::create_dir_all(root.join("human/constraints")).unwrap();
    fs::create_dir_all(root.join("validation")).unwrap();
    fs::create_dir_all(root.join("llm/src")).unwrap();
    fs::create_dir_all(root.join("llm/tests")).unwrap();
    fs::write(
        root.join("human/glossary.yaml"),
        "schema_version: 1\ntypes: {}\nenums: {}\n",
    )
    .unwrap();
    fs::write(
        root.join("validation/gates-policy.yaml"),
        "version: 1.0.0\npolicy_id: TEST\ngates: []\n",
    )
    .unwrap();
    fs::write(
        root.join("llm/map.yaml"),
        "schema_version: 1\nentries: []\n",
    )
    .unwrap();

    let validation = strictness
        .map(|s| format!("validation:\n  strictness: {s}\n"))
        .unwrap_or_default();

    fs::write(
        root.join("project.yaml"),
        format!(
            r#"schema_version: 1
project: test
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
glossary_types:
  - MissingType
git:
  commit_convention: conventional
  merge_strategy: manual
{validation}"#
        ),
    )
    .unwrap();
}

#[test]
fn project_validation_strictness_and_verify_status_default() {
    let yaml = r#"
schema_version: 1
project: test
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
validation:
  strictness: relaxed
git:
  commit_convention: conventional
  merge_strategy: manual
"#;
    let project: ProjectMap = serde_yaml::from_str(yaml).unwrap();
    let validation = project.validation.expect("validation");
    assert_eq!(validation.strictness, Strictness::Relaxed);
    assert_eq!(validation.verify_status, VerifyStatus::NotRun);
}

#[test]
fn waiver_file_parses_required_fields() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("validation")).unwrap();
    fs::write(
        tmp.path().join("validation/waivers.yaml"),
        r#"waivers:
  - code: CTR-060
    file: human/milestones/001-init/contracts/admin.md
    reason: migrated legacy contract, glossary cleanup scheduled
    expires: 2026-07-01
"#,
    )
    .unwrap();

    let waivers = WaiverFile::load(&tmp.path().join("validation/waivers.yaml")).unwrap();
    assert_eq!(waivers.waivers.len(), 1);
    assert_eq!(waivers.waivers[0].code, "CTR-060");
    assert_eq!(
        waivers.waivers[0].expires,
        NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()
    );
}

#[test]
fn strict_check_promotes_warnings_to_errors() {
    let tmp = TempDir::new().unwrap();
    write_minimal_project(tmp.path(), None);

    let report = get_check_report(
        tmp.path(),
        CheckOptions {
            strict: true,
            with_waivers: false,
        },
    )
    .unwrap();

    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.code == "PRJ-030" && matches!(d.severity, Severity::Error)));
    assert_eq!(report.exit_code, 1);
}

#[test]
fn check_report_preserves_glossary_parse_errors() {
    let tmp = TempDir::new().unwrap();
    write_minimal_project(tmp.path(), None);
    fs::write(tmp.path().join("human/glossary.yaml"), "types: [").unwrap();

    let report = get_check_report(
        tmp.path(),
        CheckOptions {
            strict: false,
            with_waivers: false,
        },
    )
    .unwrap();

    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.code == "GLO-001" && matches!(d.severity, Severity::Error)));
    assert_eq!(report.exit_code, 1);
}

#[test]
fn check_report_runs_gate_commands_for_json_and_watch_paths() {
    let tmp = TempDir::new().unwrap();
    write_minimal_project(tmp.path(), None);
    fs::write(
        tmp.path().join("validation/gates-policy.yaml"),
        r#"version: 1.0.0
policy_id: TEST
gates:
  - id: GATE-FAIL-001
    type: test
    mandatory: true
    command: definitely-not-a-real-hlv-gate-binary
"#,
    )
    .unwrap();

    let report = get_check_report(
        tmp.path(),
        CheckOptions {
            strict: false,
            with_waivers: false,
        },
    )
    .unwrap();

    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.code == "GAT-050" && matches!(d.severity, Severity::Error)));
    assert_eq!(report.exit_code, 1);
}

#[test]
fn check_with_waivers_can_suppress_gate_command_failures() {
    let tmp = TempDir::new().unwrap();
    write_minimal_project(tmp.path(), None);
    fs::write(
        tmp.path().join("validation/gates-policy.yaml"),
        r#"version: 1.0.0
policy_id: TEST
gates:
  - id: GATE-FAIL-001
    type: test
    mandatory: true
    command: definitely-not-a-real-hlv-gate-binary
"#,
    )
    .unwrap();
    fs::write(
        tmp.path().join("validation/waivers.yaml"),
        r#"waivers:
  - code: GAT-050
    file: validation/gates-policy.yaml
    reason: temporary failing gate for migration
    expires: 2099-01-01
"#,
    )
    .unwrap();

    let report = get_check_report(
        tmp.path(),
        CheckOptions {
            strict: false,
            with_waivers: true,
        },
    )
    .unwrap();

    assert!(!report.diagnostics.iter().any(|d| d.code == "GAT-050"));
    assert!(!report.diagnostics.iter().any(|d| d.code == "WVR-030"));
    assert!(report
        .waived
        .iter()
        .any(|item| item.diagnostic.code == "GAT-050"));
    assert_eq!(report.exit_code, 0);
}

#[test]
fn relaxed_check_report_skips_gate_commands() {
    let tmp = TempDir::new().unwrap();
    write_minimal_project(tmp.path(), Some("relaxed"));
    fs::write(
        tmp.path().join("validation/gates-policy.yaml"),
        r#"version: 1.0.0
policy_id: TEST
gates:
  - id: GATE-FAIL-001
    type: test
    mandatory: true
    command: definitely-not-a-real-hlv-gate-binary
"#,
    )
    .unwrap();

    let report = get_check_report(
        tmp.path(),
        CheckOptions {
            strict: false,
            with_waivers: false,
        },
    )
    .unwrap();

    assert!(!report.diagnostics.iter().any(|d| d.code == "GAT-050"));
    assert_eq!(report.strictness, Strictness::Relaxed);
}

#[test]
fn check_with_waivers_suppresses_exact_code_and_file_only() {
    let tmp = TempDir::new().unwrap();
    write_minimal_project(tmp.path(), None);
    fs::write(
        tmp.path().join("validation/waivers.yaml"),
        r#"waivers:
  - code: PRJ-030
    file: project.yaml
    reason: legacy glossary import
    expires: 2099-01-01
"#,
    )
    .unwrap();

    let report = get_check_report(
        tmp.path(),
        CheckOptions {
            strict: false,
            with_waivers: true,
        },
    )
    .unwrap();

    assert!(!report.diagnostics.iter().any(|d| d.code == "PRJ-030"));
    assert_eq!(report.waived.len(), 1);
    assert_eq!(report.exit_code, 0);
}

#[test]
fn expired_waiver_is_reported_and_does_not_suppress() {
    let tmp = TempDir::new().unwrap();
    write_minimal_project(tmp.path(), None);
    fs::write(
        tmp.path().join("validation/waivers.yaml"),
        r#"waivers:
  - code: PRJ-030
    file: project.yaml
    reason: old migration
    expires: 2000-01-01
"#,
    )
    .unwrap();

    let report = get_check_report(
        tmp.path(),
        CheckOptions {
            strict: false,
            with_waivers: true,
        },
    )
    .unwrap();

    assert!(report.diagnostics.iter().any(|d| d.code == "PRJ-030"));
    assert!(report.diagnostics.iter().any(|d| d.code == "WVR-020"));
    assert!(report.waived.is_empty());
}

#[test]
fn strict_mode_promotes_waiver_warnings_too() {
    let tmp = TempDir::new().unwrap();
    write_minimal_project(tmp.path(), None);
    fs::write(
        tmp.path().join("validation/waivers.yaml"),
        r#"waivers:
  - code: PRJ-030
    file: project.yaml
    reason: expired
    expires: 2020-01-01
"#,
    )
    .unwrap();

    let report = get_check_report(
        tmp.path(),
        CheckOptions {
            strict: true,
            with_waivers: true,
        },
    )
    .unwrap();

    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.code == "WVR-020" && matches!(d.severity, Severity::Error)));
    assert_eq!(report.exit_code, 1);
}

#[test]
fn doctor_reports_missing_project_without_root_error() {
    let tmp = TempDir::new().unwrap();

    let report = doctor_report(tmp.path(), false).unwrap();

    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.code == "DOC-001" && matches!(d.severity, Severity::Error)));
    assert_eq!(report.exit_code, 1);
}

#[test]
fn doctor_fix_creates_missing_directories_only() {
    let tmp = TempDir::new().unwrap();
    write_minimal_project(tmp.path(), None);
    fs::remove_dir_all(tmp.path().join("llm/tests")).unwrap();

    let report = doctor_report(tmp.path(), true).unwrap();

    assert!(tmp.path().join("llm/tests").is_dir());
    assert!(report.fixed.iter().any(|p| p == "llm/tests/"));
}

#[test]
fn explain_registry_finds_known_code_case_insensitively() {
    let explanation = lookup_diagnostic("ctr-060").expect("CTR-060 explanation");

    assert_eq!(explanation.code, "CTR-060");
    assert!(explanation.title.contains("Glossary"));
    assert!(!explanation.common_causes.is_empty());
    assert!(!explanation.fixes.is_empty());
}

#[test]
fn explain_registry_finds_gate_command_failure() {
    let explanation = lookup_diagnostic("gat-050").expect("GAT-050 explanation");

    assert_eq!(explanation.code, "GAT-050");
    assert!(explanation.title.contains("Gate"));
}

#[test]
fn explain_registry_finds_index_stale_code() {
    let explanation = lookup_diagnostic("idx-010").expect("IDX-010 explanation");

    assert_eq!(explanation.code, "IDX-010");
    assert!(explanation.title.contains("Signature index"));
    assert!(explanation
        .fixes
        .iter()
        .any(|fix| fix.contains("index build")));
}

#[test]
fn explain_registry_finds_test_spec_and_traceability_codes() {
    for code in ["CTR-010", "TST-020", "TST-021", "TRC-022"] {
        let explanation = lookup_diagnostic(code).expect("diagnostic explanation");
        assert_eq!(explanation.code, code);
        assert!(
            !explanation.common_causes.is_empty(),
            "{code} should list common causes"
        );
        assert!(!explanation.fixes.is_empty(), "{code} should list fixes");
    }
}

#[test]
fn explain_registry_suggests_same_prefix_for_unknown_code() {
    let suggestions = hlv::cmd::explain::suggest_diagnostics("CTR-999");

    assert!(suggestions.iter().any(|item| item.code == "CTR-060"));
}

#[test]
fn waiver_audit_reports_unmatched_waiver() {
    let tmp = TempDir::new().unwrap();
    write_minimal_project(tmp.path(), None);
    fs::write(
        tmp.path().join("validation/waivers.yaml"),
        r#"waivers:
  - code: CTR-060
    file: human/missing.md
    reason: stale waiver
    expires: 2099-01-01
"#,
    )
    .unwrap();

    let audit = audit_waivers(tmp.path()).unwrap();

    assert!(audit
        .diagnostics
        .iter()
        .any(|d| d.code == "WVR-030" && matches!(d.severity, Severity::Warning)));
    assert_eq!(audit.exit_code, 1);
}

#[test]
fn map_isolation_distinguishes_source_and_test_paths() {
    let tmp = TempDir::new().unwrap();
    write_minimal_project(tmp.path(), None);
    fs::create_dir_all(tmp.path().join("apps/backend/src")).unwrap();
    fs::write(tmp.path().join("apps/backend/src/auth.ts"), "").unwrap();
    fs::write(
        tmp.path().join("llm/map.yaml"),
        r#"schema_version: 1
entries:
  - path: apps/backend/src/auth.ts
    kind: file
    layer: llm
    description: generated auth implementation
"#,
    )
    .unwrap();

    let (diagnostics, _) = hlv::cmd::check::get_check_diagnostics(tmp.path()).unwrap();

    assert!(diagnostics.iter().any(|d| d.code == "MAP-080"));
}

#[test]
fn artifact_ownership_outside_llm_paths_is_diagnostic() {
    let tmp = TempDir::new().unwrap();
    write_minimal_project(tmp.path(), None);
    let mut project = ProjectMap::load(&tmp.path().join("project.yaml")).unwrap();
    project.artifact_graph = Some(hlv::model::project::ArtifactGraphConfig {
        code_ownership: [(
            "tests-auth".to_string(),
            hlv::model::project::CodeOwnershipEntry {
                paths: vec!["apps/backend/tests/**".to_string()],
                owners: vec!["qa".to_string()],
                verifies: vec!["spec-auth".to_string()],
                requires: vec![],
                implements: vec![],
                documents: vec![],
                depends_on: vec![],
            },
        )]
        .into_iter()
        .collect(),
    });
    project.save(&tmp.path().join("project.yaml")).unwrap();

    let (diagnostics, _) = hlv::cmd::check::get_check_diagnostics(tmp.path()).unwrap();

    assert!(diagnostics.iter().any(|d| d.code == "MAP-081"));
}

#[test]
fn display_width_helpers_handle_cjk_and_combining_marks() {
    assert_eq!(display_width("abc"), 3);
    assert_eq!(display_width("語"), 2);
    assert_eq!(display_width("e\u{301}"), 1);

    let padded = pad_display_width("語", 4);
    assert_eq!(display_width(&padded), 4);

    let truncated = truncate_display_width("ab語cd", 5);
    assert_eq!(display_width(&truncated), 5);
    assert!(truncated.ends_with('…'));
}

#[test]
fn diagnostic_print_width_helper_can_pad_non_ascii_messages() {
    let diag = Diagnostic::warning("DOC-999", "Проверка 語");
    let padded = pad_display_width(&diag.message, 20);

    assert_eq!(display_width(&padded), 20);
}
