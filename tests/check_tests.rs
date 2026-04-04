use std::fs;
use std::path::Path;
use tempfile::TempDir;

use hlv::check::code_trace::check_code_trace;
use hlv::check::constraints::check_constraints;
use hlv::check::contracts::check_contracts;
use hlv::check::llm_map::check_llm_map;
use hlv::model::project::LlmPaths;
use hlv::check::plan::check_stage_plans;
use hlv::check::project_map::check_project_map;
use hlv::check::sec_markers::check_sec_markers;
use hlv::check::stack::check_stack;
use hlv::check::traceability::check_traceability;
use hlv::check::validation::check_test_specs;
use hlv::check::{self, Severity};
use hlv::model::glossary::Glossary;
use hlv::model::project::ConstraintEntry;
use hlv::model::project::{
    ComponentType, ContractEntry, ContractStatus, DependencyType, Stack, StackComponent,
    StackDependency,
};

// ═══════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════

fn make_entry(id: &str, path: &str) -> ContractEntry {
    ContractEntry {
        id: id.to_string(),
        version: "1.0.0".to_string(),
        path: path.to_string(),
        yaml_path: None,
        owner: None,
        status: ContractStatus::Draft,
        test_spec: None,
        depends_on: vec![],
        artifacts: vec![],
    }
}

fn empty_glossary() -> Glossary {
    Glossary {
        schema_version: None,
        domain: None,
        types: Default::default(),
        enums: Default::default(),
        terms: Default::default(),
        rules: vec![],
    }
}

fn has_error(diags: &[check::Diagnostic], code: &str) -> bool {
    diags
        .iter()
        .any(|d| d.code == code && matches!(d.severity, Severity::Error))
}

fn has_warning(diags: &[check::Diagnostic], code: &str) -> bool {
    diags
        .iter()
        .any(|d| d.code == code && matches!(d.severity, Severity::Warning))
}

fn has_info(diags: &[check::Diagnostic], code: &str) -> bool {
    diags
        .iter()
        .any(|d| d.code == code && matches!(d.severity, Severity::Info))
}

fn has_any_error(diags: &[check::Diagnostic]) -> bool {
    diags.iter().any(|d| matches!(d.severity, Severity::Error))
}

fn count_errors(diags: &[check::Diagnostic]) -> usize {
    diags
        .iter()
        .filter(|d| matches!(d.severity, Severity::Error))
        .count()
}

fn count_warnings(diags: &[check::Diagnostic]) -> usize {
    diags
        .iter()
        .filter(|d| matches!(d.severity, Severity::Warning))
        .count()
}

// ═══════════════════════════════════════════════════════
// check::exit_code
// ═══════════════════════════════════════════════════════

#[test]
fn exit_code_zero_on_empty() {
    assert_eq!(check::exit_code(&[]), 0);
}

#[test]
fn exit_code_zero_on_info_only() {
    let diags = vec![check::Diagnostic::info("X", "msg")];
    assert_eq!(check::exit_code(&diags), 0);
}

#[test]
fn exit_code_zero_on_warning() {
    let diags = vec![check::Diagnostic::warning("X", "msg")];
    assert_eq!(check::exit_code(&diags), 0);
}

#[test]
fn exit_code_one_on_error() {
    let diags = vec![check::Diagnostic::error("X", "msg")];
    assert_eq!(check::exit_code(&diags), 1);
}

#[test]
fn exit_code_error_wins_over_warning() {
    let diags = vec![
        check::Diagnostic::warning("W", "warn"),
        check::Diagnostic::error("E", "err"),
    ];
    assert_eq!(check::exit_code(&diags), 1);
}

// ═══════════════════════════════════════════════════════
// check::contracts — MD contracts
// ═══════════════════════════════════════════════════════

fn write_valid_contract(dir: &Path, name: &str) {
    let contracts_dir = dir.join("human/milestones/001/contracts");
    fs::create_dir_all(&contracts_dir).unwrap();
    fs::create_dir_all(dir.join("human/milestones/001/artifacts")).unwrap();

    let md = format!(
        r#"# {name} v1.0.0
owner: team

## Sources
- [task](../artifacts/task.md) — main requirement

## Intent
Do something useful.

## Input
```yaml
type: object
properties:
  id:
    type: string
```

## Output
```yaml
type: object
properties:
  result:
    type: string
```

## Errors
| Code | HTTP | When |
|------|------|------|
| NOT_FOUND | 404 | Item missing |

## Invariants
1. Result must always be present.

## Examples
### Happy path
```json
{{"input": {{"id": "1"}}, "output": {{"result": "ok"}}}}
```
### Error case
```json
{{"input": {{"id": ""}}, "error": "NOT_FOUND"}}
```

## Edge Cases
None.

## NFR
```yaml
latency_p99_ms: 200
```

## Security
Auth required.
"#
    );
    fs::write(contracts_dir.join(format!("{name}.md")), md).unwrap();
    fs::write(dir.join("human/milestones/001/artifacts/task.md"), "task").unwrap();
}

#[test]
fn contracts_valid_passes() {
    let tmp = TempDir::new().unwrap();
    write_valid_contract(tmp.path(), "order.create");
    let entry = make_entry(
        "order.create",
        "human/milestones/001/contracts/order.create.md",
    );
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(!has_any_error(&diags), "unexpected errors: {:?}", diags);
}

#[test]
fn contracts_missing_file_error() {
    let tmp = TempDir::new().unwrap();
    let entry = make_entry(
        "order.create",
        "human/milestones/001/contracts/order.create.md",
    );
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(has_error(&diags, "CTR-001"));
}

#[test]
fn contracts_missing_sections_error() {
    let tmp = TempDir::new().unwrap();
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    fs::create_dir_all(&contracts_dir).unwrap();
    // Only header, no sections
    fs::write(contracts_dir.join("bad.md"), "# bad v1.0.0\n\nJust text.\n").unwrap();
    let entry = make_entry("bad", "human/milestones/001/contracts/bad.md");
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_error(&diags, "CTR-010"),
        "expected missing section errors: {:?}",
        diags
    );
}

#[test]
fn contracts_missing_header_error() {
    let tmp = TempDir::new().unwrap();
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    fs::create_dir_all(&contracts_dir).unwrap();
    // No proper header
    fs::write(
        contracts_dir.join("nohead.md"),
        "No header here\n## Intent\nSomething\n",
    )
    .unwrap();
    let entry = make_entry("nohead", "human/milestones/001/contracts/nohead.md");
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_error(&diags, "CTR-002") || has_error(&diags, "CTR-003"),
        "expected header errors: {:?}",
        diags
    );
}

#[test]
fn contracts_missing_input_yaml() {
    let tmp = TempDir::new().unwrap();
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    fs::create_dir_all(&contracts_dir).unwrap();

    // Minimal contract — all required sections present but Input has no yaml fenced block
    let md = r#"# test v1.0.0

## Sources
None

## Intent
Test

## Input
Just text.

## Output
Just text.

## Errors
None

## Invariants
None

## Examples
None

## Edge Cases
None

## NFR
None

## Security
None
"#;
    fs::write(contracts_dir.join("test.md"), md).unwrap();
    let entry = make_entry("test", "human/milestones/001/contracts/test.md");
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_error(&diags, "CTR-031"),
        "expected missing input yaml: {:?}",
        diags
    );
}

#[test]
fn contracts_no_examples_warnings() {
    let tmp = TempDir::new().unwrap();
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    fs::create_dir_all(&contracts_dir).unwrap();

    let md = r#"# test v1.0.0

## Sources
None

## Intent
Test

## Input
```yaml
type: object
```

## Output
```yaml
type: object
```

## Errors
| Code | HTTP | When |
|------|------|------|
| ERR | 500 | always |

## Invariants
1. Something

## Examples
No actual json blocks.

## Edge Cases
None

## NFR
```yaml
latency_p99_ms: 100
```

## Security
None
"#;
    fs::write(contracts_dir.join("test.md"), md).unwrap();
    let entry = make_entry("test", "human/milestones/001/contracts/test.md");
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_warning(&diags, "CTR-040"),
        "expected no happy path warning: {:?}",
        diags
    );
}

// ── CTR-020: source file not found ──────────────────────
#[test]
fn contracts_source_file_not_found() {
    let tmp = TempDir::new().unwrap();
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    fs::create_dir_all(&contracts_dir).unwrap();
    // Do NOT create the artifact file referenced in Sources
    let md = r#"# test v1.0.0
owner: team

## Sources
- [task](../artifacts/missing.md) — missing ref

## Intent
Test

## Input
```yaml
type: object
```

## Output
```yaml
type: object
```

## Errors
| Code | HTTP | When |
|------|------|------|
| ERR | 500 | always |

## Invariants
1. Something

## Examples
### Happy path
```json
{"input": {"id": "1"}, "output": {"result": "ok"}}
```
### Error case
```json
{"input": {"id": ""}, "error": "ERR"}
```

## Edge Cases
None

## NFR
```yaml
latency_p99_ms: 100
```

## Security
None
"#;
    fs::write(contracts_dir.join("test.md"), md).unwrap();
    let entry = make_entry("test", "human/milestones/001/contracts/test.md");
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_warning(&diags, "CTR-020"),
        "expected source not found warning: {:?}",
        diags
    );
}

// ── CTR-030: invalid input YAML ──────────────────────
#[test]
fn contracts_invalid_input_yaml() {
    let tmp = TempDir::new().unwrap();
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    fs::create_dir_all(&contracts_dir).unwrap();
    let md = r#"# test v1.0.0
owner: team

## Sources
None

## Intent
Test

## Input
```yaml
{{{invalid
```

## Output
```yaml
type: object
```

## Errors
| Code | HTTP | When |
|------|------|------|
| ERR | 500 | always |

## Invariants
1. Something

## Examples
### Happy path
```json
{"input": {}, "output": {}}
```
### Error case
```json
{"input": {}, "error": "ERR"}
```

## Edge Cases
None

## NFR
None

## Security
None
"#;
    fs::write(contracts_dir.join("test.md"), md).unwrap();
    let entry = make_entry("test", "human/milestones/001/contracts/test.md");
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_error(&diags, "CTR-030"),
        "expected invalid input yaml: {:?}",
        diags
    );
}

// ── CTR-032: invalid output YAML ──────────────────────
#[test]
fn contracts_invalid_output_yaml() {
    let tmp = TempDir::new().unwrap();
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    fs::create_dir_all(&contracts_dir).unwrap();
    let md = r#"# test v1.0.0
owner: team

## Sources
None

## Intent
Test

## Input
```yaml
type: object
```

## Output
```yaml
{{{invalid
```

## Errors
| Code | HTTP | When |
|------|------|------|
| ERR | 500 | always |

## Invariants
1. Something

## Examples
### Happy path
```json
{"input": {}, "output": {}}
```
### Error case
```json
{"input": {}, "error": "ERR"}
```

## Edge Cases
None

## NFR
None

## Security
None
"#;
    fs::write(contracts_dir.join("test.md"), md).unwrap();
    let entry = make_entry("test", "human/milestones/001/contracts/test.md");
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_error(&diags, "CTR-032"),
        "expected invalid output yaml: {:?}",
        diags
    );
}

// ── CTR-041: no error example ──────────────────────
#[test]
fn contracts_no_error_example() {
    let tmp = TempDir::new().unwrap();
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    fs::create_dir_all(&contracts_dir).unwrap();
    let md = r#"# test v1.0.0
owner: team

## Sources
None

## Intent
Test

## Input
```yaml
type: object
```

## Output
```yaml
type: object
```

## Errors
| Code | HTTP | When |
|------|------|------|
| ERR | 500 | always |

## Invariants
1. Something

## Examples
### Happy path
```json
{"input": {"id": "1"}, "output": {"result": "ok"}}
```

## Edge Cases
None

## NFR
```yaml
latency_p99_ms: 100
```

## Security
None
"#;
    fs::write(contracts_dir.join("test.md"), md).unwrap();
    let entry = make_entry("test", "human/milestones/001/contracts/test.md");
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_warning(&diags, "CTR-041"),
        "expected no error example warning: {:?}",
        diags
    );
}

// ── CTR-050: empty errors table ──────────────────────
#[test]
fn contracts_empty_errors_table() {
    let tmp = TempDir::new().unwrap();
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    fs::create_dir_all(&contracts_dir).unwrap();
    let md = r#"# test v1.0.0
owner: team

## Sources
None

## Intent
Test

## Input
```yaml
type: object
```

## Output
```yaml
type: object
```

## Errors
No errors defined.

## Invariants
1. Something

## Examples
### Happy path
```json
{"input": {}, "output": {}}
```
### Error case
```json
{"input": {}, "error": "ERR"}
```

## Edge Cases
None

## NFR
None

## Security
None
"#;
    fs::write(contracts_dir.join("test.md"), md).unwrap();
    let entry = make_entry("test", "human/milestones/001/contracts/test.md");
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_warning(&diags, "CTR-050"),
        "expected empty errors warning: {:?}",
        diags
    );
}

// ── CTR-051: no invariants ──────────────────────
#[test]
fn contracts_no_invariants() {
    let tmp = TempDir::new().unwrap();
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    fs::create_dir_all(&contracts_dir).unwrap();
    let md = r#"# test v1.0.0
owner: team

## Sources
None

## Intent
Test

## Input
```yaml
type: object
```

## Output
```yaml
type: object
```

## Errors
| Code | HTTP | When |
|------|------|------|
| ERR | 500 | always |

## Invariants

## Examples
### Happy path
```json
{"input": {}, "output": {}}
```
### Error case
```json
{"input": {}, "error": "ERR"}
```

## Edge Cases
None

## NFR
None

## Security
None
"#;
    fs::write(contracts_dir.join("test.md"), md).unwrap();
    let entry = make_entry("test", "human/milestones/001/contracts/test.md");
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_warning(&diags, "CTR-051"),
        "expected no invariants warning: {:?}",
        diags
    );
}

// ── CTR-060: unknown glossary ref ──────────────────────
#[test]
fn contracts_unknown_glossary_ref() {
    let tmp = TempDir::new().unwrap();
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    fs::create_dir_all(&contracts_dir).unwrap();
    let md = r#"# test v1.0.0
owner: team

## Sources
None

## Intent
Test

## Input
```yaml
type: object
properties:
  order:
    $ref: "glossary#FakeType"
```

## Output
```yaml
type: object
```

## Errors
| Code | HTTP | When |
|------|------|------|
| ERR | 500 | always |

## Invariants
1. Something

## Examples
### Happy path
```json
{"input": {}, "output": {}}
```
### Error case
```json
{"input": {}, "error": "ERR"}
```

## Edge Cases
None

## NFR
None

## Security
None
"#;
    fs::write(contracts_dir.join("test.md"), md).unwrap();
    let entry = make_entry("test", "human/milestones/001/contracts/test.md");
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_warning(&diags, "CTR-060"),
        "expected unknown glossary ref warning: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════
// check::contracts — YAML contracts
// ═══════════════════════════════════════════════════════

#[test]
fn contracts_yaml_valid_passes() {
    let tmp = TempDir::new().unwrap();
    write_valid_contract(tmp.path(), "svc.op");
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");

    let yaml = r#"id: svc.op
version: 1.0.0
owner: team
intent: "Do something"
inputs_schema:
  type: object
outputs_schema:
  type: object
errors:
  - code: ERR_1
    http_status: 400
invariants:
  - id: INV-001
    expr: "always true"
nfr:
  latency_p99_ms: 200
security:
  - rule: auth_required
"#;
    fs::write(contracts_dir.join("svc.op.yaml"), yaml).unwrap();

    let mut entry = make_entry("svc.op", "human/milestones/001/contracts/svc.op.md");
    entry.yaml_path = Some("human/milestones/001/contracts/svc.op.yaml".to_string());
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());

    let yaml_errors: Vec<_> = diags
        .iter()
        .filter(|d| d.code.starts_with("CTR-Y"))
        .collect();
    assert!(
        yaml_errors.is_empty(),
        "unexpected YAML errors: {:?}",
        yaml_errors
    );
}

#[test]
fn contracts_yaml_id_mismatch() {
    let tmp = TempDir::new().unwrap();
    write_valid_contract(tmp.path(), "svc.op");
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");

    let yaml = r#"id: wrong.id
version: 1.0.0
inputs_schema:
  type: object
outputs_schema:
  type: object
errors: []
invariants: []
"#;
    fs::write(contracts_dir.join("svc.op.yaml"), yaml).unwrap();

    let mut entry = make_entry("svc.op", "human/milestones/001/contracts/svc.op.md");
    entry.yaml_path = Some("human/milestones/001/contracts/svc.op.yaml".to_string());
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_error(&diags, "CTR-Y10"),
        "expected id mismatch: {:?}",
        diags
    );
}

// ── CTR-Y02: missing id ──────────────────────
#[test]
fn contracts_yaml_missing_id() {
    let tmp = TempDir::new().unwrap();
    write_valid_contract(tmp.path(), "svc.op");
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    let yaml = r#"id: ""
version: 1.0.0
inputs_schema:
  type: object
outputs_schema:
  type: object
errors: []
invariants: []
"#;
    fs::write(contracts_dir.join("svc.op.yaml"), yaml).unwrap();
    let mut entry = make_entry("svc.op", "human/milestones/001/contracts/svc.op.md");
    entry.yaml_path = Some("human/milestones/001/contracts/svc.op.yaml".to_string());
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_error(&diags, "CTR-Y02"),
        "expected missing id: {:?}",
        diags
    );
}

// ── CTR-Y03: missing version ──────────────────────
#[test]
fn contracts_yaml_missing_version() {
    let tmp = TempDir::new().unwrap();
    write_valid_contract(tmp.path(), "svc.op");
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    let yaml = r#"id: svc.op
version: ""
inputs_schema:
  type: object
outputs_schema:
  type: object
errors: []
invariants: []
"#;
    fs::write(contracts_dir.join("svc.op.yaml"), yaml).unwrap();
    let mut entry = make_entry("svc.op", "human/milestones/001/contracts/svc.op.md");
    entry.yaml_path = Some("human/milestones/001/contracts/svc.op.yaml".to_string());
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_error(&diags, "CTR-Y03"),
        "expected missing version: {:?}",
        diags
    );
}

// ── CTR-Y20: no inputs_schema ──────────────────────
#[test]
fn contracts_yaml_no_inputs_schema() {
    let tmp = TempDir::new().unwrap();
    write_valid_contract(tmp.path(), "svc.op");
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    let yaml = r#"id: svc.op
version: 1.0.0
outputs_schema:
  type: object
errors: []
invariants: []
"#;
    fs::write(contracts_dir.join("svc.op.yaml"), yaml).unwrap();
    let mut entry = make_entry("svc.op", "human/milestones/001/contracts/svc.op.md");
    entry.yaml_path = Some("human/milestones/001/contracts/svc.op.yaml".to_string());
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_error(&diags, "CTR-Y20"),
        "expected no inputs_schema: {:?}",
        diags
    );
}

// ── CTR-Y21: no outputs_schema ──────────────────────
#[test]
fn contracts_yaml_no_outputs_schema() {
    let tmp = TempDir::new().unwrap();
    write_valid_contract(tmp.path(), "svc.op");
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    let yaml = r#"id: svc.op
version: 1.0.0
inputs_schema:
  type: object
errors: []
invariants: []
"#;
    fs::write(contracts_dir.join("svc.op.yaml"), yaml).unwrap();
    let mut entry = make_entry("svc.op", "human/milestones/001/contracts/svc.op.md");
    entry.yaml_path = Some("human/milestones/001/contracts/svc.op.yaml".to_string());
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_error(&diags, "CTR-Y21"),
        "expected no outputs_schema: {:?}",
        diags
    );
}

// ── CTR-Y22: empty errors ──────────────────────
#[test]
fn contracts_yaml_empty_errors_warning() {
    let tmp = TempDir::new().unwrap();
    write_valid_contract(tmp.path(), "svc.op");
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    let yaml = r#"id: svc.op
version: 1.0.0
inputs_schema:
  type: object
outputs_schema:
  type: object
errors: []
invariants:
  - id: INV-001
    expr: "always"
"#;
    fs::write(contracts_dir.join("svc.op.yaml"), yaml).unwrap();
    let mut entry = make_entry("svc.op", "human/milestones/001/contracts/svc.op.md");
    entry.yaml_path = Some("human/milestones/001/contracts/svc.op.yaml".to_string());
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_warning(&diags, "CTR-Y22"),
        "expected empty errors warning: {:?}",
        diags
    );
}

// ── CTR-Y23: empty invariants ──────────────────────
#[test]
fn contracts_yaml_empty_invariants_warning() {
    let tmp = TempDir::new().unwrap();
    write_valid_contract(tmp.path(), "svc.op");
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    let yaml = r#"id: svc.op
version: 1.0.0
inputs_schema:
  type: object
outputs_schema:
  type: object
errors:
  - code: ERR_1
    http_status: 400
invariants: []
"#;
    fs::write(contracts_dir.join("svc.op.yaml"), yaml).unwrap();
    let mut entry = make_entry("svc.op", "human/milestones/001/contracts/svc.op.md");
    entry.yaml_path = Some("human/milestones/001/contracts/svc.op.yaml".to_string());
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_warning(&diags, "CTR-Y23"),
        "expected empty invariants warning: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════
// check::plan
// ═══════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════
// check::traceability
// ═══════════════════════════════════════════════════════

fn write_traceability(dir: &Path, yaml: &str) {
    fs::create_dir_all(dir.join("human")).unwrap();
    fs::write(dir.join("human/traceability.yaml"), yaml).unwrap();
}

#[test]
fn traceability_valid_passes() {
    let tmp = TempDir::new().unwrap();
    write_traceability(
        tmp.path(),
        r#"
schema_version: 1
requirements:
  - id: REQ-001
    statement: "Create order"
mappings:
  - requirement: REQ-001
    contracts: [order.create]
    tests: [CT-001]
    runtime_gates: [GATE-CONTRACT-001]
"#,
    );

    // Set up project.yaml + gates policy + test spec so cross-reference checks pass
    fs::create_dir_all(tmp.path().join("validation/test-specs")).unwrap();
    fs::create_dir_all(tmp.path().join("human/milestones/001/contracts")).unwrap();
    fs::create_dir_all(tmp.path().join("human/constraints")).unwrap();
    fs::write(
        tmp.path().join("validation/gates-policy.yaml"),
        "version: 1.0.0\npolicy_id: X\ngates:\n  - id: GATE-CONTRACT-001\n    type: contract_tests\n    mandatory: true\n",
    )
    .unwrap();
    fs::write(
        tmp.path().join("project.yaml"),
        r#"schema_version: 1
project: test
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
"#,
    )
    .unwrap();

    // Create test spec with CT-001
    fs::write(
        tmp.path().join("validation/test-specs/order.create.md"),
        "# Test Spec\n\nderived_from: c.md\n\n## Tests\n\n### CT-001: Create order test\n",
    )
    .unwrap();

    let mut entry = make_entry("order.create", "c.md");
    entry.test_spec = Some("validation/test-specs/order.create.md".to_string());
    let entries = vec![entry];
    let diags = check_traceability(tmp.path(), "human/traceability.yaml", &entries);
    assert_eq!(count_errors(&diags), 0, "unexpected errors: {:?}", diags);
    assert_eq!(
        count_warnings(&diags),
        0,
        "unexpected warnings: {:?}",
        diags
    );
}

#[test]
fn traceability_unknown_contract_error() {
    let tmp = TempDir::new().unwrap();
    write_traceability(
        tmp.path(),
        r#"
schema_version: 1
requirements:
  - id: REQ-001
    statement: "Test"
mappings:
  - requirement: REQ-001
    contracts: [nonexistent.contract]
    tests: [CT-001]
    runtime_gates: [GATE-001]
"#,
    );
    let diags = check_traceability(tmp.path(), "human/traceability.yaml", &[]);
    assert!(
        has_error(&diags, "TRC-010"),
        "expected unknown contract: {:?}",
        diags
    );
}

#[test]
fn traceability_unknown_requirement_error() {
    let tmp = TempDir::new().unwrap();
    write_traceability(
        tmp.path(),
        r#"
schema_version: 1
requirements:
  - id: REQ-001
    statement: "Test"
mappings:
  - requirement: REQ-999
    contracts: [order.create]
    tests: [CT-001]
    runtime_gates: [GATE-001]
"#,
    );
    let entries = vec![make_entry("order.create", "c.md")];
    let diags = check_traceability(tmp.path(), "human/traceability.yaml", &entries);
    assert!(has_error(&diags, "TRC-011"));
}

#[test]
fn traceability_no_tests_warning() {
    let tmp = TempDir::new().unwrap();
    write_traceability(
        tmp.path(),
        r#"
schema_version: 1
requirements:
  - id: REQ-001
    statement: "Test"
mappings:
  - requirement: REQ-001
    contracts: [order.create]
    tests: []
    runtime_gates: [GATE-001]
"#,
    );
    let entries = vec![make_entry("order.create", "c.md")];
    let diags = check_traceability(tmp.path(), "human/traceability.yaml", &entries);
    assert!(has_warning(&diags, "TRC-020"));
}

#[test]
fn traceability_unmapped_requirement_warning() {
    let tmp = TempDir::new().unwrap();
    write_traceability(
        tmp.path(),
        r#"
schema_version: 1
requirements:
  - id: REQ-001
    statement: "Mapped"
  - id: REQ-002
    statement: "Not mapped"
mappings:
  - requirement: REQ-001
    contracts: [order.create]
    tests: [CT-001]
    runtime_gates: [GATE-001]
"#,
    );
    let entries = vec![make_entry("order.create", "c.md")];
    let diags = check_traceability(tmp.path(), "human/traceability.yaml", &entries);
    assert!(has_warning(&diags, "TRC-030"));
}

#[test]
fn traceability_unparseable_error() {
    let tmp = TempDir::new().unwrap();
    write_traceability(tmp.path(), "not: [valid: yaml: {{{}}}");
    let diags = check_traceability(tmp.path(), "human/traceability.yaml", &[]);
    assert!(has_error(&diags, "TRC-001"));
}

// ── TRC-021: no gates warning ──────────────────────
#[test]
fn traceability_no_gates_warning() {
    let tmp = TempDir::new().unwrap();
    write_traceability(
        tmp.path(),
        r#"
schema_version: 1
requirements:
  - id: REQ-001
    statement: "Test"
mappings:
  - requirement: REQ-001
    contracts: [order.create]
    tests: [CT-001]
    runtime_gates: []
"#,
    );
    let entries = vec![make_entry("order.create", "c.md")];
    let diags = check_traceability(tmp.path(), "human/traceability.yaml", &entries);
    assert!(
        has_warning(&diags, "TRC-021"),
        "expected no gates warning: {:?}",
        diags
    );
}

#[test]
fn traceability_unknown_test_reference_warning() {
    let tmp = TempDir::new().unwrap();
    write_traceability(
        tmp.path(),
        r#"
schema_version: 1
requirements:
  - id: REQ-001
    statement: "Test"
mappings:
  - requirement: REQ-001
    contracts: [order.create]
    tests: [NONEXISTENT-TEST-001]
    runtime_gates: []
"#,
    );
    let entries = vec![make_entry("order.create", "c.md")];
    let diags = check_traceability(tmp.path(), "human/traceability.yaml", &entries);
    assert!(
        has_warning(&diags, "TRC-022"),
        "expected unknown test warning: {:?}",
        diags
    );
}

#[test]
fn traceability_unknown_gate_reference_warning() {
    let tmp = TempDir::new().unwrap();
    write_traceability(
        tmp.path(),
        r#"
schema_version: 1
requirements:
  - id: REQ-001
    statement: "Test"
mappings:
  - requirement: REQ-001
    contracts: [order.create]
    tests: []
    runtime_gates: [NONEXISTENT-GATE]
"#,
    );
    let entries = vec![make_entry("order.create", "c.md")];
    let diags = check_traceability(tmp.path(), "human/traceability.yaml", &entries);
    assert!(
        has_warning(&diags, "TRC-023"),
        "expected unknown gate warning: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════
// check::validation (test specs)
// ═══════════════════════════════════════════════════════

fn write_test_spec(dir: &Path, name: &str, content: &str) {
    let specs_dir = dir.join("validation/test-specs");
    fs::create_dir_all(&specs_dir).unwrap();
    fs::write(specs_dir.join(format!("{name}.md")), content).unwrap();
}

#[test]
fn test_specs_version_drift_warning() {
    let tmp = TempDir::new().unwrap();
    let spec = r#"# Test Spec: order.create
derived_from: human/milestones/001/contracts/order.create.md
contract_version: 1.0.0

## Contract Tests

### CT-001: Happy path
Gate: GATE-CONTRACT-001

## Property-Based Tests

### PBT-001: Invariant
Gate: GATE-PBT-001
"#;
    write_test_spec(tmp.path(), "order.create", spec);
    let mut entry = make_entry(
        "order.create",
        "human/milestones/001/contracts/order.create.md",
    );
    entry.version = "2.0.0".to_string(); // contract upgraded, test spec still says 1.0.0
    entry.test_spec = Some("validation/test-specs/order.create.md".to_string());
    let diags = check_test_specs(tmp.path(), &[entry]);
    assert!(
        has_warning(&diags, "TST-011"),
        "expected version drift warning: {:?}",
        diags
    );
}

#[test]
fn test_specs_version_match_no_warning() {
    let tmp = TempDir::new().unwrap();
    let spec = r#"# Test Spec: order.create
derived_from: human/milestones/001/contracts/order.create.md
contract_version: 1.0.0

## Contract Tests

### CT-001: Happy path
Gate: GATE-CONTRACT-001

## Property-Based Tests

### PBT-001: Invariant
Gate: GATE-PBT-001
"#;
    write_test_spec(tmp.path(), "order.create", spec);
    let mut entry = make_entry(
        "order.create",
        "human/milestones/001/contracts/order.create.md",
    );
    entry.test_spec = Some("validation/test-specs/order.create.md".to_string());
    let diags = check_test_specs(tmp.path(), &[entry]);
    assert!(
        !has_warning(&diags, "TST-011"),
        "version matches, should not warn: {:?}",
        diags
    );
}

#[test]
fn test_specs_valid_passes() {
    let tmp = TempDir::new().unwrap();
    let spec = r#"# Test Spec: order.create
derived_from: human/milestones/001/contracts/order.create.md

## Contract Tests

### CT-001: Happy path
Gate: GATE-CONTRACT-001

### CT-002: Error case
Gate: GATE-CONTRACT-001

## Property-Based Tests

### PBT-001: Invariant check
Gate: GATE-PBT-001
"#;
    write_test_spec(tmp.path(), "order.create", spec);
    let mut entry = make_entry(
        "order.create",
        "human/milestones/001/contracts/order.create.md",
    );
    entry.test_spec = Some("validation/test-specs/order.create.md".to_string());
    let diags = check_test_specs(tmp.path(), &[entry]);
    assert_eq!(count_errors(&diags), 0, "unexpected errors: {:?}", diags);
    assert_eq!(
        count_warnings(&diags),
        0,
        "unexpected warnings: {:?}",
        diags
    );
}

#[test]
fn test_specs_missing_file_error() {
    let tmp = TempDir::new().unwrap();
    let mut entry = make_entry("order.create", "c.md");
    entry.test_spec = Some("validation/test-specs/nonexistent.md".to_string());
    let diags = check_test_specs(tmp.path(), &[entry]);
    assert!(has_error(&diags, "TST-002"));
}

#[test]
fn test_specs_no_spec_warning() {
    let tmp = TempDir::new().unwrap();
    let entry = make_entry("order.create", "c.md"); // no test_spec field
    let diags = check_test_specs(tmp.path(), &[entry]);
    assert!(has_warning(&diags, "TST-001"));
}

#[test]
fn test_specs_no_contract_tests_warning() {
    let tmp = TempDir::new().unwrap();
    let spec = r#"# Test Spec: order.create
derived_from: human/milestones/001/contracts/order.create.md

## Some Section
Just text, no CT-* or PBT-* tests.
GATE-CONTRACT-001 referenced.
"#;
    write_test_spec(tmp.path(), "order.create", spec);
    let mut entry = make_entry(
        "order.create",
        "human/milestones/001/contracts/order.create.md",
    );
    entry.test_spec = Some("validation/test-specs/order.create.md".to_string());
    let diags = check_test_specs(tmp.path(), &[entry]);
    assert!(
        has_warning(&diags, "TST-020"),
        "expected no CT warning: {:?}",
        diags
    );
}

#[test]
fn test_specs_no_gate_refs_warning() {
    let tmp = TempDir::new().unwrap();
    let spec = r#"# Test Spec: order.create
derived_from: human/milestones/001/contracts/order.create.md

## Contract Tests
### CT-001: Test
No gate here.

## Property-Based Tests
### PBT-001: Invariant
Still no gate.
"#;
    write_test_spec(tmp.path(), "order.create", spec);
    let mut entry = make_entry(
        "order.create",
        "human/milestones/001/contracts/order.create.md",
    );
    entry.test_spec = Some("validation/test-specs/order.create.md".to_string());
    let diags = check_test_specs(tmp.path(), &[entry]);
    assert!(has_warning(&diags, "TST-030"));
}

#[test]
fn test_specs_duplicate_test_id_error() {
    let tmp = TempDir::new().unwrap();
    let spec1 = r#"# Test Spec: a
derived_from: human/milestones/001/contracts/a.md
## Tests
### CT-001: First
GATE-CONTRACT-001
### PBT-001: Prop
"#;
    let spec2 = r#"# Test Spec: b
derived_from: human/milestones/001/contracts/b.md
## Tests
### CT-001: Duplicate ID!
GATE-CONTRACT-001
### PBT-002: Prop
"#;
    write_test_spec(tmp.path(), "a", spec1);
    write_test_spec(tmp.path(), "b", spec2);

    let mut e1 = make_entry("a", "human/milestones/001/contracts/a.md");
    e1.test_spec = Some("validation/test-specs/a.md".to_string());
    let mut e2 = make_entry("b", "human/milestones/001/contracts/b.md");
    e2.test_spec = Some("validation/test-specs/b.md".to_string());

    let diags = check_test_specs(tmp.path(), &[e1, e2]);
    assert!(
        has_error(&diags, "TST-040"),
        "expected duplicate ID: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════
// check::project_map
// ═══════════════════════════════════════════════════════

fn scaffold_project(dir: &Path) {
    fs::create_dir_all(dir.join("human/milestones/001/contracts")).unwrap();
    fs::create_dir_all(dir.join("human/constraints")).unwrap();
    fs::create_dir_all(dir.join("validation/test-specs")).unwrap();
    fs::write(
        dir.join("human/glossary.yaml"),
        "schema_version: 1\ntypes: {}\nenums: {}\nterms: {}\nrules: []\n",
    )
    .unwrap();
    fs::write(
        dir.join("validation/gates-policy.yaml"),
        "version: 1.0.0\npolicy_id: X\ngates: []\n",
    )
    .unwrap();
    fs::write(
        dir.join("human/traceability.yaml"),
        "schema_version: 1\nrequirements: []\nmappings: []\n",
    )
    .unwrap();
}

#[test]
fn project_map_valid_passes() {
    let tmp = TempDir::new().unwrap();
    scaffold_project(tmp.path());

    let yaml = r#"schema_version: 1
project: test
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
"#;
    fs::write(tmp.path().join("project.yaml"), yaml).unwrap();

    let diags = check_project_map(tmp.path());
    // Only path errors for optional dirs that don't exist (artifacts, scenarios, etc)
    let critical: Vec<_> = diags
        .iter()
        .filter(|d| {
            matches!(d.severity, Severity::Error)
                && !d.message.contains("human/artifacts")
                && !d.message.contains("scenarios")
                && !d.message.contains("plan")
        })
        .collect();
    assert!(
        critical.is_empty(),
        "unexpected critical errors: {:?}",
        critical
    );
}

// ── PRJ-080: paths.llm.src must be under llm/ ──────────────────────
#[test]
fn project_map_llm_src_outside_llm() {
    let tmp = TempDir::new().unwrap();
    scaffold_project(tmp.path());
    let yaml = r#"schema_version: 1
project: test
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
    src: src/
    tests: tests/
"#;
    fs::write(tmp.path().join("project.yaml"), yaml).unwrap();
    let diags = check_project_map(tmp.path());
    assert!(
        has_error(&diags, "PRJ-080"),
        "expected error for src outside llm/: {:?}",
        diags
    );
    assert!(
        has_error(&diags, "PRJ-081"),
        "expected error for tests outside llm/: {:?}",
        diags
    );
}

#[test]
fn project_map_missing_yaml_error() {
    let tmp = TempDir::new().unwrap();
    // No project.yaml at all
    let diags = check_project_map(tmp.path());
    assert!(has_error(&diags, "PRJ-001"));
}

#[test]
fn project_map_missing_glossary_error() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("human/milestones/001/contracts")).unwrap();
    fs::create_dir_all(tmp.path().join("human/constraints")).unwrap();
    fs::create_dir_all(tmp.path().join("validation/test-specs")).unwrap();
    fs::write(
        tmp.path().join("validation/gates-policy.yaml"),
        "version: 1.0.0\npolicy_id: X\ngates: []\n",
    )
    .unwrap();
    // No glossary.yaml

    let yaml = r#"schema_version: 1
project: test
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
"#;
    fs::write(tmp.path().join("project.yaml"), yaml).unwrap();
    let diags = check_project_map(tmp.path());
    assert!(
        has_error(&diags, "PRJ-010"),
        "expected missing glossary: {:?}",
        diags
    );
}

#[test]
fn project_map_unknown_glossary_type_warning() {
    let tmp = TempDir::new().unwrap();
    scaffold_project(tmp.path());

    let yaml = r#"schema_version: 1
project: test
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
glossary_types:
  - NonExistentType
"#;
    fs::write(tmp.path().join("project.yaml"), yaml).unwrap();

    let diags = check_project_map(tmp.path());
    assert!(
        has_warning(&diags, "PRJ-030"),
        "expected unknown glossary type: {:?}",
        diags
    );
}

// ── TST-010: derived_from mismatch ──────────────────────
#[test]
fn test_specs_derived_from_mismatch() {
    let tmp = TempDir::new().unwrap();
    let spec = r#"# Test Spec: something.else
derived_from: human/milestones/001/contracts/something.else.md

## Contract Tests

### CT-101: Happy path
Gate: GATE-CONTRACT-001

## Property-Based Tests

### PBT-101: Invariant check
Gate: GATE-PBT-001
"#;
    write_test_spec(tmp.path(), "order.create", spec);
    let mut entry = make_entry(
        "order.create",
        "human/milestones/001/contracts/order.create.md",
    );
    entry.test_spec = Some("validation/test-specs/order.create.md".to_string());
    let diags = check_test_specs(tmp.path(), &[entry]);
    assert!(
        has_warning(&diags, "TST-010"),
        "expected derived_from mismatch: {:?}",
        diags
    );
}

// ── TST-021: no PBT tests ──────────────────────
#[test]
fn test_specs_no_pbt_warning() {
    let tmp = TempDir::new().unwrap();
    let spec = r#"# Test Spec: order.create
derived_from: human/milestones/001/contracts/order.create.md

## Contract Tests

### CT-001: Happy path
Gate: GATE-CONTRACT-001

### CT-002: Error case
Gate: GATE-CONTRACT-001
"#;
    write_test_spec(tmp.path(), "order.create", spec);
    let mut entry = make_entry(
        "order.create",
        "human/milestones/001/contracts/order.create.md",
    );
    entry.test_spec = Some("validation/test-specs/order.create.md".to_string());
    let diags = check_test_specs(tmp.path(), &[entry]);
    assert!(
        has_warning(&diags, "TST-021"),
        "expected no PBT warning: {:?}",
        diags
    );
}

// ── PRJ-012: missing constraints dir ──────────────────────
#[test]
fn project_map_missing_constraints_dir() {
    let tmp = TempDir::new().unwrap();
    scaffold_project(tmp.path());
    fs::remove_dir(tmp.path().join("human/constraints")).unwrap();
    let yaml = r#"schema_version: 1
project: test
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
"#;
    fs::write(tmp.path().join("project.yaml"), yaml).unwrap();
    let diags = check_project_map(tmp.path());
    assert!(
        has_error(&diags, "PRJ-012"),
        "expected missing constraints dir: {:?}",
        diags
    );
}

// ── PRJ-014: missing gates policy ──────────────────────
#[test]
fn project_map_missing_gates_policy() {
    let tmp = TempDir::new().unwrap();
    scaffold_project(tmp.path());
    fs::remove_file(tmp.path().join("validation/gates-policy.yaml")).unwrap();
    let yaml = r#"schema_version: 1
project: test
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
"#;
    fs::write(tmp.path().join("project.yaml"), yaml).unwrap();
    let diags = check_project_map(tmp.path());
    assert!(
        has_error(&diags, "PRJ-014"),
        "expected missing gates policy: {:?}",
        diags
    );
}

// ── PRJ-040: missing constraint path ──────────────────────
#[test]
fn project_map_missing_constraint_path() {
    let tmp = TempDir::new().unwrap();
    scaffold_project(tmp.path());
    let yaml = r#"schema_version: 1
project: test
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
constraints:
  - id: PERF-001
    path: human/constraints/nonexistent.yaml
"#;
    fs::write(tmp.path().join("project.yaml"), yaml).unwrap();
    let diags = check_project_map(tmp.path());
    assert!(
        has_error(&diags, "PRJ-040"),
        "expected missing constraint path: {:?}",
        diags
    );
}

// ── PRJ-070: verify passed but status not promoted ──────────────────────
// ═══════════════════════════════════════════════════════
// find_project_root
// ═══════════════════════════════════════════════════════

#[test]
fn find_root_explicit_valid() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("project.yaml"),
        "schema_version: 1\nproject: t\nstatus: draft\n",
    )
    .unwrap();
    let root = hlv::find_project_root(Some(tmp.path().to_str().unwrap())).unwrap();
    assert_eq!(root, tmp.path());
}

#[test]
fn find_root_explicit_missing() {
    let tmp = TempDir::new().unwrap();
    let result = hlv::find_project_root(Some(tmp.path().to_str().unwrap()));
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════
// cmd::init — idempotency
// ═══════════════════════════════════════════════════════

#[test]
fn init_creates_scaffold() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    hlv::cmd::init::run_with_milestone(
        path,
        Some("myproject"),
        Some("myteam"),
        Some("claude"),
        Some("init"),
        Some("standard"),
    )
    .unwrap();

    assert!(tmp.path().join("project.yaml").exists());
    assert!(tmp.path().join("milestones.yaml").exists());
    assert!(tmp.path().join("HLV.md").exists());
    assert!(tmp.path().join("AGENTS.md").exists());
    assert!(tmp.path().join("human/glossary.yaml").exists());
    assert!(tmp.path().join("human/milestones").is_dir());
    assert!(tmp.path().join("validation/gates-policy.yaml").exists());
    assert!(tmp
        .path()
        .join(".claude/skills/artifacts/SKILL.md")
        .exists());
    assert!(tmp.path().join(".claude/skills/generate/SKILL.md").exists());
    assert!(tmp
        .path()
        .join(".claude/skills/implement/SKILL.md")
        .exists());
    assert!(!tmp.path().join(".claude/skills/init/SKILL.md").exists()); // init skill removed
                                                                        // First milestone created
    assert!(tmp
        .path()
        .join("human/milestones/001-init/artifacts")
        .is_dir());
}

#[test]
fn init_idempotent() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    hlv::cmd::init::run_with_milestone(
        path,
        Some("myproject"),
        Some("myteam"),
        Some("claude"),
        Some("init"),
        Some("standard"),
    )
    .unwrap();

    let original = fs::read_to_string(tmp.path().join("project.yaml")).unwrap();

    // Run again — should not overwrite (reinit mode)
    hlv::cmd::init::run(path, Some("different"), Some("other"), Some("claude"), None).unwrap();

    let after = fs::read_to_string(tmp.path().join("project.yaml")).unwrap();
    assert_eq!(
        original, after,
        "project.yaml was overwritten on second init"
    );
}

#[test]
fn init_different_agent() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    hlv::cmd::init::run_with_milestone(
        path,
        Some("proj"),
        Some("team"),
        Some("copilot"),
        Some("init"),
        Some("minimal"),
    )
    .unwrap();

    assert!(tmp
        .path()
        .join(".copilot/skills/generate/SKILL.md")
        .exists());
    assert!(!tmp.path().join(".claude/skills").exists());
}

// ═══════════════════════════════════════════════════════
// check::stack
// ═══════════════════════════════════════════════════════

fn make_stack_dep(name: &str, dep_type: DependencyType) -> StackDependency {
    StackDependency {
        name: name.to_string(),
        dependency_type: dep_type,
        version: None,
    }
}

fn make_stack_component(
    id: &str,
    ct: ComponentType,
    langs: &[&str],
    deps: Vec<StackDependency>,
) -> StackComponent {
    StackComponent {
        id: id.to_string(),
        component_type: ct,
        languages: langs.iter().map(|s| s.to_string()).collect(),
        dependencies: deps,
        extra: std::collections::HashMap::new(),
    }
}

#[test]
fn stack_empty_warning() {
    let stack = Stack { components: vec![] };
    let diags = check_stack(&stack);
    assert!(
        has_warning(&diags, "STK-001"),
        "expected empty stack warning: {:?}",
        diags
    );
}

#[test]
fn stack_missing_component_id() {
    let stack = Stack {
        components: vec![make_stack_component(
            "",
            ComponentType::Service,
            &["rust"],
            vec![],
        )],
    };
    let diags = check_stack(&stack);
    assert!(
        has_error(&diags, "STK-010"),
        "expected missing id error: {:?}",
        diags
    );
}

#[test]
fn stack_duplicate_component_id() {
    let stack = Stack {
        components: vec![
            make_stack_component("api", ComponentType::Service, &["rust"], vec![]),
            make_stack_component("api", ComponentType::Library, &["rust"], vec![]),
        ],
    };
    let diags = check_stack(&stack);
    assert!(
        has_error(&diags, "STK-011"),
        "expected duplicate id error: {:?}",
        diags
    );
}

#[test]
fn stack_missing_languages_warning() {
    let stack = Stack {
        components: vec![make_stack_component(
            "api",
            ComponentType::Service,
            &[],
            vec![],
        )],
    };
    let diags = check_stack(&stack);
    assert!(
        has_warning(&diags, "STK-012"),
        "expected missing languages warning: {:?}",
        diags
    );
}

#[test]
fn stack_no_language_warning_for_infra_types() {
    let infra_types = vec![
        ("postgres", ComponentType::Datastore),
        ("stripe", ComponentType::ExternalApi),
        ("slack", ComponentType::Channel),
        ("vercel", ComponentType::Hosting),
    ];
    for (id, ct) in infra_types {
        let stack = Stack {
            components: vec![make_stack_component(id, ct, &[], vec![])],
        };
        let diags = check_stack(&stack);
        assert!(
            !has_warning(&diags, "STK-012"),
            "{} should not warn about missing languages: {:?}",
            id,
            diags
        );
    }
}

#[test]
fn stack_language_warning_for_code_types() {
    let code_types = vec![
        ("api", ComponentType::Service),
        ("lib", ComponentType::Library),
        ("tool", ComponentType::Cli),
        ("setup", ComponentType::Script),
        ("app", ComponentType::Application),
    ];
    for (id, ct) in code_types {
        let stack = Stack {
            components: vec![make_stack_component(id, ct, &[], vec![])],
        };
        let diags = check_stack(&stack);
        assert!(
            has_warning(&diags, "STK-012"),
            "{} should warn about missing languages: {:?}",
            id,
            diags
        );
    }
}

#[test]
fn stack_dependency_missing_name() {
    let stack = Stack {
        components: vec![make_stack_component(
            "api",
            ComponentType::Service,
            &["rust"],
            vec![make_stack_dep("", DependencyType::Framework)],
        )],
    };
    let diags = check_stack(&stack);
    assert!(
        has_error(&diags, "STK-020"),
        "expected missing dep name error: {:?}",
        diags
    );
}

#[test]
fn stack_duplicate_dependency_name() {
    let stack = Stack {
        components: vec![make_stack_component(
            "api",
            ComponentType::Service,
            &["rust"],
            vec![
                make_stack_dep("axum", DependencyType::Framework),
                make_stack_dep("axum", DependencyType::Framework),
            ],
        )],
    };
    let diags = check_stack(&stack);
    assert!(
        has_warning(&diags, "STK-021"),
        "expected duplicate dep warning: {:?}",
        diags
    );
}

#[test]
fn stack_valid_passes() {
    let stack = Stack {
        components: vec![
            make_stack_component(
                "backend",
                ComponentType::Service,
                &["rust"],
                vec![
                    make_stack_dep("axum", DependencyType::Framework),
                    make_stack_dep("tokio", DependencyType::Runtime),
                ],
            ),
            make_stack_component(
                "migrations",
                ComponentType::Script,
                &["sql"],
                vec![make_stack_dep("sqlx-cli", DependencyType::Tool)],
            ),
        ],
    };
    let diags = check_stack(&stack);
    assert_eq!(count_errors(&diags), 0, "unexpected errors: {:?}", diags);
    assert_eq!(
        count_warnings(&diags),
        0,
        "unexpected warnings: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════
// Regression tests for audit findings
// ═══════════════════════════════════════════════════════

// Bug 3: MD version mismatch with project.yaml
#[test]
fn contracts_md_version_mismatch() {
    let tmp = TempDir::new().unwrap();
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    fs::create_dir_all(&contracts_dir).unwrap();
    // MD says v2.0.0 but entry says 1.0.0
    let md = r#"# test v2.0.0
owner: team

## Sources
None

## Intent
Test

## Input
```yaml
type: object
```

## Output
```yaml
type: object
```

## Errors
| Code | HTTP | When | Source |
|------|------|------|--------|
| ERR | 500 | always | test |

## Invariants
1. Something

## Examples
### Happy path
```json
{"input": {}, "output": {}}
```
### Error case
```json
{"input": {}, "error": "ERR"}
```

## Edge Cases
None

## NFR
None

## Security
None
"#;
    fs::write(contracts_dir.join("test.md"), md).unwrap();
    let entry = make_entry("test", "human/milestones/001/contracts/test.md"); // version = "1.0.0"
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_error(&diags, "CTR-004"),
        "expected version mismatch error: {:?}",
        diags
    );
}

// Bug 3: YAML version mismatch with project.yaml
#[test]
fn contracts_yaml_version_mismatch() {
    let tmp = TempDir::new().unwrap();
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    fs::create_dir_all(&contracts_dir).unwrap();
    let yaml = r#"id: test
version: "2.0.0"
inputs_schema:
  type: object
outputs_schema:
  type: object
errors:
  - code: ERR
invariants:
  - id: INV-1
"#;
    fs::write(contracts_dir.join("test.yaml"), yaml).unwrap();
    let mut entry = make_entry("test", "human/milestones/001/contracts/test.md");
    entry.yaml_path = Some("human/milestones/001/contracts/test.yaml".to_string());
    // entry.version is "1.0.0", YAML says "2.0.0"
    // We only check YAML here, skip MD by making it absent (CTR-001 will fire but we don't care)
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    assert!(
        has_error(&diags, "CTR-Y11"),
        "expected YAML version mismatch error: {:?}",
        diags
    );
}

// Bug 4: Output YAML must come from Output section, not from NFR
#[test]
fn contracts_output_yaml_not_from_nfr() {
    let tmp = TempDir::new().unwrap();
    let contracts_dir = tmp.path().join("human/milestones/001/contracts");
    fs::create_dir_all(&contracts_dir).unwrap();
    // Contract with YAML in Input and NFR, but NO YAML in Output
    let md = r#"# test v1.0.0
owner: team

## Sources
None

## Intent
Test

## Input
```yaml
type: object
required: [user_id]
```

## Output
No YAML here — just prose.

## Errors
| Code | HTTP | When | Source |
|------|------|------|--------|
| ERR | 500 | always | test |

## Invariants
1. Something

## Examples
### Happy path
```json
{"input": {}, "output": {}}
```
### Error case
```json
{"input": {}, "error": "ERR"}
```

## Edge Cases
None

## NFR
```yaml
latency_p99_ms: 200
```

## Security
None
    "#;
    fs::write(contracts_dir.join("test.md"), md).unwrap();
    let entry = make_entry("test", "human/milestones/001/contracts/test.md");
    let diags = check_contracts(tmp.path(), &[entry], &empty_glossary());
    // Output section has no YAML, so output_yaml should be None.
    // Previously the NFR YAML was mistakenly treated as Output.
    // CTR-031 should NOT fire (that's "No Input YAML"), but there should be no
    // false positive from NFR leaking into output_yaml.
    // The contract MD parser should return output_yaml = None.
    // We check that the contract validates: no CTR-032 (invalid output yaml) fires.
    // We could also check that output_yaml is None via the parser directly.
    use hlv::model::contract_md::ContractMd;
    let text = fs::read_to_string(contracts_dir.join("test.md")).unwrap();
    let contract = ContractMd::from_markdown(&text);
    assert!(
        contract.output_yaml.is_none(),
        "output_yaml should be None when Output section has no YAML block, got: {:?}",
        contract.output_yaml
    );
    assert!(
        contract.input_yaml.is_some(),
        "input_yaml should still be parsed"
    );
    assert!(
        contract.nfr_yaml.is_some(),
        "nfr_yaml should still be parsed"
    );
    assert!(
        has_error(&diags, "CTR-033"),
        "expected missing output YAML diagnostic: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════
// check::plan — PLN-* diagnostics
// ═══════════════════════════════════════════════════════

fn setup_milestone_dir(root: &Path, milestone_id: &str) {
    fs::create_dir_all(root.join("human/milestones").join(milestone_id)).unwrap();
}

fn write_stage(root: &Path, milestone_id: &str, num: u32, content: &str) {
    let dir = root.join("human/milestones").join(milestone_id);
    fs::write(dir.join(format!("stage_{}.md", num)), content).unwrap();
}

#[test]
fn pln001_no_stage_files() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_milestone_dir(root, "001-test");

    let contracts = vec![make_entry("order.create", "contracts/order.create.md")];
    let diags = check_stage_plans(root, "001-test", &contracts);
    assert!(
        has_info(&diags, "PLN-001"),
        "expected PLN-001 info: {:?}",
        diags
    );
}

#[test]
fn pln010_unreadable_stage_file() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_milestone_dir(root, "001-test");

    // Create a stage file that's a directory (unreadable as file)
    fs::create_dir_all(root.join("human/milestones/001-test/stage_1.md")).unwrap();

    let diags = check_stage_plans(root, "001-test", &[]);
    assert!(
        has_error(&diags, "PLN-010"),
        "expected PLN-010 error for unreadable stage: {:?}",
        diags
    );
}

#[test]
fn pln010_duplicate_task_id_across_stages() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_milestone_dir(root, "001-test");

    write_stage(
        root,
        "001-test",
        1,
        r#"# Stage 1: First

## Tasks

TASK-001 Do something
  contracts: [order.create]
  output: llm/src/a/
"#,
    );

    write_stage(
        root,
        "001-test",
        2,
        r#"# Stage 2: Second

## Tasks

TASK-001 Duplicate ID
  contracts: [order.cancel]
  output: llm/src/b/
"#,
    );

    let contracts = vec![
        make_entry("order.create", "c/order.create.md"),
        make_entry("order.cancel", "c/order.cancel.md"),
    ];
    let diags = check_stage_plans(root, "001-test", &contracts);
    assert!(
        has_error(&diags, "PLN-010"),
        "expected PLN-010 error for duplicate task ID: {:?}",
        diags
    );
}

#[test]
fn pln020_dependency_cycle() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_milestone_dir(root, "001-test");

    write_stage(
        root,
        "001-test",
        1,
        r#"# Stage 1: Cycle

## Tasks

TASK-001 A
  depends_on: [TASK-002]
  contracts: [x]
  output: llm/src/a/

TASK-002 B
  depends_on: [TASK-001]
  contracts: [x]
  output: llm/src/b/
"#,
    );

    let contracts = vec![make_entry("x", "c/x.md")];
    let diags = check_stage_plans(root, "001-test", &contracts);
    assert!(
        has_error(&diags, "PLN-020"),
        "expected PLN-020 cycle error: {:?}",
        diags
    );
}

#[test]
fn pln040_contract_not_covered() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_milestone_dir(root, "001-test");

    write_stage(
        root,
        "001-test",
        1,
        r#"# Stage 1: Partial

## Tasks

TASK-001 Only one contract
  contracts: [order.create]
  output: llm/src/a/
"#,
    );

    let contracts = vec![
        make_entry("order.create", "c/order.create.md"),
        make_entry("order.cancel", "c/order.cancel.md"),
    ];
    let diags = check_stage_plans(root, "001-test", &contracts);
    assert!(
        has_warning(&diags, "PLN-040"),
        "expected PLN-040 warning for uncovered contract: {:?}",
        diags
    );
}

#[test]
fn pln_happy_path_all_contracts_covered() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_milestone_dir(root, "001-test");

    write_stage(
        root,
        "001-test",
        1,
        r#"# Stage 1: Foundation (~25K)

## Contracts
- order.create
- order.cancel

## Tasks

TASK-001 Domain
  contracts: [order.create, order.cancel]
  output: llm/src/domain/

TASK-002 Create handler
  depends_on: [TASK-001]
  contracts: [order.create]
  output: llm/src/features/create/

TASK-003 Cancel handler
  depends_on: [TASK-001]
  contracts: [order.cancel]
  output: llm/src/features/cancel/

## Remediation
"#,
    );

    let contracts = vec![
        make_entry("order.create", "c/order.create.md"),
        make_entry("order.cancel", "c/order.cancel.md"),
    ];
    let diags = check_stage_plans(root, "001-test", &contracts);
    assert!(
        !has_any_error(&diags),
        "expected no errors in happy path: {:?}",
        diags
    );
    assert!(
        !has_warning(&diags, "PLN-040"),
        "all contracts should be covered: {:?}",
        diags
    );
}

#[test]
fn pln_cross_stage_deps_allowed() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_milestone_dir(root, "001-test");

    write_stage(
        root,
        "001-test",
        1,
        r#"# Stage 1: Base

## Tasks

TASK-001 Foundation
  contracts: [a]
  output: llm/src/a/
"#,
    );

    // Stage 2 depends on TASK-001 from stage 1 — should not be an error
    write_stage(
        root,
        "001-test",
        2,
        r#"# Stage 2: Extension

## Tasks

TASK-002 Depends on stage 1
  depends_on: [TASK-001]
  contracts: [a]
  output: llm/src/b/
"#,
    );

    let contracts = vec![make_entry("a", "c/a.md")];
    let diags = check_stage_plans(root, "001-test", &contracts);
    assert!(
        !has_error(&diags, "PLN-020"),
        "cross-stage deps should not create cycle errors: {:?}",
        diags
    );
}

#[test]
fn pln_with_fixture_milestone_project() {
    let root = Path::new("tests/fixtures/milestone-project");
    // The fixture has contracts dir but we need to supply contract entries
    let contracts = vec![
        make_entry(
            "order.create",
            "human/milestones/001-checkout/contracts/order.create.md",
        ),
        make_entry(
            "order.cancel",
            "human/milestones/001-checkout/contracts/order.cancel.md",
        ),
    ];
    let diags = check_stage_plans(root, "001-checkout", &contracts);
    assert!(
        !has_any_error(&diags),
        "fixture milestone-project should pass plan checks: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════
// check::llm_map — MAP-* integration tests
// ═══════════════════════════════════════════════════════

fn default_llm_paths() -> LlmPaths {
    LlmPaths {
        src: "llm/src/".to_string(),
        tests: Some("llm/tests/".to_string()),
        map: Some("llm/map.yaml".to_string()),
    }
}

fn flat_llm_paths() -> LlmPaths {
    LlmPaths {
        src: "src/".to_string(),
        tests: Some("tests/".to_string()),
        map: Some("map.yaml".to_string()),
    }
}

#[test]
fn map001_missing_map_file() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let diags = check_llm_map(root, "llm/map.yaml", &default_llm_paths());
    assert!(
        has_error(&diags, "MAP-001"),
        "expected MAP-001 when map file is missing: {:?}",
        diags
    );
}

#[test]
fn map002_invalid_yaml() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("llm")).unwrap();
    fs::write(root.join("llm/map.yaml"), "not: [valid: yaml: {{{").unwrap();

    let diags = check_llm_map(root, "llm/map.yaml", &default_llm_paths());
    assert!(
        has_error(&diags, "MAP-002"),
        "expected MAP-002 for invalid YAML: {:?}",
        diags
    );
}

#[test]
fn map003_empty_entries() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("llm")).unwrap();
    fs::write(
        root.join("llm/map.yaml"),
        "schema_version: 1\nentries: []\n",
    )
    .unwrap();

    let diags = check_llm_map(root, "llm/map.yaml", &default_llm_paths());
    assert!(
        has_info(&diags, "MAP-003"),
        "expected MAP-003 info for empty entries: {:?}",
        diags
    );
}

#[test]
fn map010_forward_missing_file() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("llm")).unwrap();
    fs::write(
        root.join("llm/map.yaml"),
        r#"schema_version: 1
entries:
  - path: llm/src/main.rs
    kind: file
    layer: llm
    description: "Entry point"
"#,
    )
    .unwrap();

    let diags = check_llm_map(root, "llm/map.yaml", &default_llm_paths());
    assert!(
        has_error(&diags, "MAP-010"),
        "expected MAP-010 for missing file on disk: {:?}",
        diags
    );
}

#[test]
fn map020_reverse_unlisted_file() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/listed.rs"), "").unwrap();
    fs::write(root.join("src/unlisted.rs"), "").unwrap();

    fs::write(
        root.join("map.yaml"),
        r#"schema_version: 1
entries:
  - path: src/
    kind: dir
    layer: llm
    description: "Source"
  - path: src/listed.rs
    kind: file
    layer: llm
    description: "Listed"
"#,
    )
    .unwrap();

    let diags = check_llm_map(root, "map.yaml", &flat_llm_paths());
    assert!(
        has_warning(&diags, "MAP-020"),
        "expected MAP-020 for unlisted file: {:?}",
        diags
    );
    // Verify it mentions the unlisted file
    let map020_diags: Vec<_> = diags.iter().filter(|d| d.code == "MAP-020").collect();
    assert!(
        map020_diags
            .iter()
            .any(|d| d.message.contains("unlisted.rs")),
        "MAP-020 should mention unlisted.rs: {:?}",
        map020_diags
    );
}

#[test]
fn map100_forward_summary() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

    fs::write(
        root.join("map.yaml"),
        r#"schema_version: 1
entries:
  - path: src/
    kind: dir
    layer: llm
    description: "Source"
  - path: src/main.rs
    kind: file
    layer: llm
    description: "Entry"
"#,
    )
    .unwrap();

    let diags = check_llm_map(root, "map.yaml", &flat_llm_paths());
    assert!(
        has_info(&diags, "MAP-100"),
        "expected MAP-100 forward summary: {:?}",
        diags
    );
    let map100 = diags.iter().find(|d| d.code == "MAP-100").unwrap();
    assert!(
        map100.message.contains("2/2"),
        "expected 2/2 in forward summary: {}",
        map100.message
    );
}

#[test]
fn map101_reverse_all_clean() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

    fs::write(
        root.join("map.yaml"),
        r#"schema_version: 1
entries:
  - path: src/
    kind: dir
    layer: llm
    description: "Source"
  - path: src/main.rs
    kind: file
    layer: llm
    description: "Entry"
"#,
    )
    .unwrap();

    let diags = check_llm_map(root, "map.yaml", &flat_llm_paths());
    assert!(
        has_info(&diags, "MAP-101"),
        "expected MAP-101 reverse summary: {:?}",
        diags
    );
    let map101 = diags.iter().find(|d| d.code == "MAP-101").unwrap();
    assert!(
        map101.message.contains("all files"),
        "expected 'all files' in reverse summary: {}",
        map101.message
    );
}

#[test]
fn map101_reverse_with_unlisted() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "").unwrap();
    fs::write(root.join("src/extra.rs"), "").unwrap();

    fs::write(
        root.join("map.yaml"),
        r#"schema_version: 1
entries:
  - path: src/
    kind: dir
    layer: llm
    description: "Source"
  - path: src/main.rs
    kind: file
    layer: llm
    description: "Entry"
"#,
    )
    .unwrap();

    let diags = check_llm_map(root, "map.yaml", &flat_llm_paths());
    let map101 = diags.iter().find(|d| d.code == "MAP-101").unwrap();
    assert!(
        map101.message.contains("1 file(s)"),
        "expected '1 file(s)' in reverse summary: {}",
        map101.message
    );
}

#[test]
fn map_with_fixture_example_project() {
    let root = Path::new("tests/fixtures/example-project");
    let diags = check_llm_map(root, "llm/map.yaml", &default_llm_paths());
    // Should at least parse and produce MAP-100 summary
    assert!(
        has_info(&diags, "MAP-100"),
        "fixture example-project should produce MAP-100: {:?}",
        diags
    );
    assert!(
        !has_error(&diags, "MAP-001"),
        "fixture should find map.yaml: {:?}",
        diags
    );
    assert!(
        !has_error(&diags, "MAP-002"),
        "fixture map.yaml should be valid YAML: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════
// check::code_trace — CTR-* integration tests
// ═══════════════════════════════════════════════════════

#[test]
fn code_trace_all_markers_present() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Contract YAML
    fs::create_dir_all(root.join("contracts")).unwrap();
    fs::write(
        root.join("contracts/order.yaml"),
        r#"id: order
version: 1.0.0
errors:
  - code: NOT_FOUND
    http_status: 404
invariants:
  - id: idempotent
inputs_schema:
  type: object
outputs_schema:
  type: object
"#,
    )
    .unwrap();

    // Source with markers
    fs::create_dir_all(root.join("llm/src")).unwrap();
    fs::write(
        root.join("llm/src/main.rs"),
        "// @hlv NOT_FOUND\n// @hlv idempotent\n",
    )
    .unwrap();

    let contracts = vec![make_entry_with_yaml(
        "order",
        "contracts/order.md",
        "contracts/order.yaml",
    )];
    let diags = check_code_trace(root, &contracts, &[], "llm/src", None, true);
    assert!(
        !diags.iter().any(|d| d.code == "CTR-010"),
        "all markers present: {:?}",
        diags
    );
}

#[test]
fn code_trace_missing_marker_warns() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::create_dir_all(root.join("contracts")).unwrap();
    fs::write(
        root.join("contracts/order.yaml"),
        r#"id: order
version: 1.0.0
errors:
  - code: MISSING_ERR
    http_status: 500
invariants: []
inputs_schema:
  type: object
outputs_schema:
  type: object
"#,
    )
    .unwrap();

    fs::create_dir_all(root.join("llm/src")).unwrap();
    fs::write(root.join("llm/src/main.rs"), "fn main() {}").unwrap();

    let contracts = vec![make_entry_with_yaml(
        "order",
        "contracts/order.md",
        "contracts/order.yaml",
    )];
    let diags = check_code_trace(root, &contracts, &[], "llm/src", None, true);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "CTR-010" && d.message.contains("MISSING_ERR")),
        "expected CTR-010 for missing marker: {:?}",
        diags
    );
}

#[test]
fn code_trace_constraint_rules() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::create_dir_all(root.join("human/constraints")).unwrap();
    fs::write(
        root.join("human/constraints/security.yaml"),
        r#"id: security
version: "1.0.0"
rules:
  - id: auth_required
    severity: critical
    statement: "Auth required"
"#,
    )
    .unwrap();

    fs::create_dir_all(root.join("llm/src")).unwrap();
    fs::write(root.join("llm/src/main.rs"), "// @hlv auth_required\n").unwrap();

    let constraints = vec![ConstraintEntry {
        id: "security".to_string(),
        path: "human/constraints/security.yaml".to_string(),
        applies_to: Some("all".to_string()),
    }];
    let diags = check_code_trace(root, &[], &constraints, "llm/src", None, true);
    assert!(
        !diags.iter().any(|d| d.code == "CTR-010"),
        "constraint marker found: {:?}",
        diags
    );
}

fn make_entry_with_yaml(id: &str, path: &str, yaml_path: &str) -> ContractEntry {
    ContractEntry {
        id: id.to_string(),
        version: "1.0.0".to_string(),
        path: path.to_string(),
        yaml_path: Some(yaml_path.to_string()),
        owner: None,
        status: ContractStatus::Draft,
        test_spec: None,
        depends_on: vec![],
        artifacts: vec![],
    }
}

// ═══════════════════════════════════════════════════════
// cmd::gates — integration tests
// ═══════════════════════════════════════════════════════

fn setup_gates_project(root: &Path) {
    // project.yaml
    fs::write(
        root.join("project.yaml"),
        r#"schema_version: 1
project: test-gates
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

    // gates policy
    fs::create_dir_all(root.join("validation")).unwrap();
    fs::write(
        root.join("validation/gates-policy.yaml"),
        r#"version: "1.0.0"
policy_id: TEST-GATES
gates:
  - id: GATE-001
    type: unit_tests
    mandatory: true
  - id: GATE-002
    type: integration
    mandatory: false
"#,
    )
    .unwrap();
}

#[test]
fn gates_enable_disable() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_gates_project(root);

    // Disable GATE-001
    hlv::cmd::gates::run_disable(root, "GATE-001").unwrap();

    let policy =
        hlv::model::policy::GatesPolicy::load(&root.join("validation/gates-policy.yaml")).unwrap();
    assert!(!policy.gates[0].enabled);

    // Re-enable
    hlv::cmd::gates::run_enable(root, "GATE-001").unwrap();
    let policy =
        hlv::model::policy::GatesPolicy::load(&root.join("validation/gates-policy.yaml")).unwrap();
    assert!(policy.gates[0].enabled);
}

#[test]
fn gates_set_clear_command() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_gates_project(root);

    hlv::cmd::gates::run_set_command(root, "GATE-001", "cargo test").unwrap();
    let policy =
        hlv::model::policy::GatesPolicy::load(&root.join("validation/gates-policy.yaml")).unwrap();
    assert_eq!(policy.gates[0].command.as_deref(), Some("cargo test"));

    hlv::cmd::gates::run_clear_command(root, "GATE-001").unwrap();
    let policy =
        hlv::model::policy::GatesPolicy::load(&root.join("validation/gates-policy.yaml")).unwrap();
    assert!(policy.gates[0].command.is_none());
}

#[test]
fn gates_set_clear_cwd() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_gates_project(root);

    hlv::cmd::gates::run_set_cwd(root, "GATE-002", "llm/src").unwrap();
    let policy =
        hlv::model::policy::GatesPolicy::load(&root.join("validation/gates-policy.yaml")).unwrap();
    assert_eq!(policy.gates[1].cwd.as_deref(), Some("llm/src"));

    hlv::cmd::gates::run_clear_cwd(root, "GATE-002").unwrap();
    let policy =
        hlv::model::policy::GatesPolicy::load(&root.join("validation/gates-policy.yaml")).unwrap();
    assert!(policy.gates[1].cwd.is_none());
}

#[test]
fn gates_invalid_gate_id() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_gates_project(root);

    let result = hlv::cmd::gates::run_enable(root, "NONEXISTENT");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn gates_run_commands_no_commands() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_gates_project(root);

    // No commands set → (0, 0, 0)
    let (passed, failed, skipped) = hlv::cmd::gates::run_gate_commands(root, None).unwrap();
    assert_eq!((passed, failed, skipped), (0, 0, 0));
}

#[test]
fn gates_run_commands_passing() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_gates_project(root);

    // Set a passing command
    hlv::cmd::gates::run_set_command(root, "GATE-001", "true").unwrap();

    // Create milestones.yaml so results can be saved
    hlv::cmd::milestone::run_new(root, "test").unwrap();

    let (passed, failed, _) = hlv::cmd::gates::run_gate_commands(root, None).unwrap();
    assert_eq!(passed, 1);
    assert_eq!(failed, 0);

    // Verify results persisted
    let ms = hlv::model::milestone::MilestoneMap::load(&root.join("milestones.yaml")).unwrap();
    let current = ms.current.unwrap();
    assert_eq!(current.gate_results.len(), 1);
    assert_eq!(
        current.gate_results[0].status,
        hlv::model::milestone::GateRunStatus::Passed
    );
}

#[test]
fn gates_run_commands_failing() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_gates_project(root);

    hlv::cmd::gates::run_set_command(root, "GATE-001", "false").unwrap();

    let (passed, failed, _) = hlv::cmd::gates::run_gate_commands(root, None).unwrap();
    assert_eq!(passed, 0);
    assert_eq!(failed, 1);
}

// ═══════════════════════════════════════════════════════
// cmd::workflow — integration tests
// ═══════════════════════════════════════════════════════

#[test]
fn workflow_no_milestones_yaml() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let result = hlv::cmd::workflow::run(root, false);
    assert!(result.is_err(), "should fail without milestones.yaml");
}

#[test]
fn workflow_no_active_milestone() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(root.join("milestones.yaml"), "project: test\nhistory: []\n").unwrap();

    // Should succeed (prints "no active milestone")
    hlv::cmd::workflow::run(root, false).unwrap();
}

#[test]
fn workflow_with_active_milestone_no_stages() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(
        root.join("milestones.yaml"),
        r#"project: test
current:
  id: 001-test
  number: 1
  stages: []
history: []
"#,
    )
    .unwrap();

    hlv::cmd::workflow::run(root, false).unwrap();
}

#[test]
fn workflow_with_stages() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(
        root.join("milestones.yaml"),
        r#"project: test
current:
  id: 001-test
  number: 1
  stage: 1
  stages:
    - id: 1
      scope: Foundation
      status: implementing
    - id: 2
      scope: Integration
      status: pending
history: []
"#,
    )
    .unwrap();

    hlv::cmd::workflow::run(root, false).unwrap();
}

// ═══════════════════════════════════════════════════════
// cmd::plan — integration tests
// ═══════════════════════════════════════════════════════

#[test]
fn plan_no_milestones() {
    let tmp = TempDir::new().unwrap();
    let result = hlv::cmd::plan::run(tmp.path(), false, false);
    assert!(result.is_err());
}

#[test]
fn plan_no_active_milestone() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(root.join("milestones.yaml"), "project: test\nhistory: []\n").unwrap();

    hlv::cmd::plan::run(root, false, false).unwrap();
}

#[test]
fn plan_with_fixture() {
    let root = Path::new("tests/fixtures/milestone-project");
    // Both visual modes should work
    hlv::cmd::plan::run(root, false, false).unwrap();
    hlv::cmd::plan::run(root, true, false).unwrap();
}

// ═══════════════════════════════════════════════════════
// cmd::status — integration tests
// ═══════════════════════════════════════════════════════

#[test]
fn status_with_milestone() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_gates_project(root);
    hlv::cmd::milestone::run_new(root, "status-test").unwrap();

    hlv::cmd::status::run(root, false).unwrap();
}

#[test]
fn status_no_active_milestone() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_gates_project(root);
    fs::write(root.join("milestones.yaml"), "project: test\nhistory: []\n").unwrap();

    hlv::cmd::status::run(root, false).unwrap();
}

// ═══════════════════════════════════════════════════════
// cmd/trace integration tests
// ═══════════════════════════════════════════════════════

#[test]
fn trace_no_milestones_file() {
    let tmp = TempDir::new().unwrap();
    let result = hlv::cmd::trace::run(tmp.path(), false, false);
    assert!(result.is_err());
}

#[test]
fn trace_no_active_milestone() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("milestones.yaml"),
        "project: test\nhistory: []\n",
    )
    .unwrap();
    // Should not panic — just prints hint
    hlv::cmd::trace::run(tmp.path(), false, false).unwrap();
}

#[test]
fn trace_no_traceability_file() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("milestones.yaml"),
        "project: test\ncurrent:\n  id: ms-001\n  number: 1\n  stages: []\n  gate_results: []\nhistory: []\n",
    )
    .unwrap();
    // Should not panic — just prints hint about missing trace
    hlv::cmd::trace::run(tmp.path(), false, false).unwrap();
}

#[test]
fn trace_with_traceability_table() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(
        root.join("milestones.yaml"),
        "project: test\ncurrent:\n  id: ms-001\n  number: 1\n  stages: []\n  gate_results: []\nhistory: []\n",
    )
    .unwrap();
    let ms_dir = root.join("human/milestones/ms-001");
    fs::create_dir_all(&ms_dir).unwrap();
    fs::write(
        ms_dir.join("traceability.yaml"),
        r#"schema_version: 1
requirements:
  - id: REQ-001
    statement: User can create order
mappings:
  - requirement: REQ-001
    contracts: [order.create]
    tests: [test_create]
    runtime_gates: [GATE-001]
"#,
    )
    .unwrap();
    hlv::cmd::trace::run(root, false, false).unwrap();
}

#[test]
fn trace_with_visual_mode() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(
        root.join("milestones.yaml"),
        "project: test\ncurrent:\n  id: ms-001\n  number: 1\n  stages: []\n  gate_results: []\nhistory: []\n",
    )
    .unwrap();
    let ms_dir = root.join("human/milestones/ms-001");
    fs::create_dir_all(&ms_dir).unwrap();
    fs::write(
        ms_dir.join("traceability.yaml"),
        "schema_version: 1\nrequirements:\n  - id: R1\n    statement: test\nmappings:\n  - requirement: R1\n    contracts: [c1]\n    tests: [t1]\n    runtime_gates: []\n",
    )
    .unwrap();
    hlv::cmd::trace::run(root, true, false).unwrap();
}

// ═══════════════════════════════════════════════════════
// cmd/plan integration tests
// ═══════════════════════════════════════════════════════

#[test]
fn plan_no_milestones_file() {
    let tmp = TempDir::new().unwrap();
    let result = hlv::cmd::plan::run(tmp.path(), false, false);
    assert!(result.is_err());
}

#[test]
fn plan_with_stages_table_mode() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(
        root.join("milestones.yaml"),
        r#"project: test
current:
  id: ms-001
  number: 1
  stage: 1
  stages:
    - id: 1
      scope: setup
      status: pending
    - id: 2
      scope: api
      status: pending
  gate_results: []
history: []
"#,
    )
    .unwrap();
    let ms_dir = root.join("human/milestones/ms-001");
    fs::create_dir_all(&ms_dir).unwrap();
    hlv::cmd::plan::run(root, false, false).unwrap();
}

#[test]
fn plan_with_stages_visual_mode() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(
        root.join("milestones.yaml"),
        r#"project: test
current:
  id: ms-001
  number: 1
  stage: 1
  stages:
    - id: 1
      scope: setup
      status: implementing
  gate_results: []
history: []
"#,
    )
    .unwrap();
    let ms_dir = root.join("human/milestones/ms-001");
    fs::create_dir_all(&ms_dir).unwrap();
    hlv::cmd::plan::run(root, true, false).unwrap();
}

#[test]
fn plan_with_plan_md_file() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(
        root.join("milestones.yaml"),
        "project: test\ncurrent:\n  id: ms-001\n  number: 1\n  stage: 1\n  stages:\n    - id: 1\n      scope: setup\n      status: pending\n  gate_results: []\nhistory: []\n",
    )
    .unwrap();
    let ms_dir = root.join("human/milestones/ms-001");
    fs::create_dir_all(&ms_dir).unwrap();
    fs::write(ms_dir.join("plan.md"), "# Plan\n\nOverview of work.\n").unwrap();
    hlv::cmd::plan::run(root, false, false).unwrap();
}

// ═══════════════════════════════════════════════════════
// cmd/workflow — per-status coverage
// ═══════════════════════════════════════════════════════

#[test]
fn workflow_with_stages_pending() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("milestones.yaml"),
        r#"project: test
current:
  id: ms-001
  number: 1
  stage: 1
  stages:
    - id: 1
      scope: setup
      status: pending
  gate_results: []
history: []
"#,
    )
    .unwrap();
    hlv::cmd::workflow::run(tmp.path(), false).unwrap();
}

#[test]
fn workflow_with_stages_verified() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("milestones.yaml"),
        r#"project: test
current:
  id: ms-001
  number: 1
  stage: 1
  stages:
    - id: 1
      scope: setup
      status: verified
  gate_results: []
history: []
"#,
    )
    .unwrap();
    hlv::cmd::workflow::run(tmp.path(), false).unwrap();
}

#[test]
fn workflow_with_stages_implementing() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("milestones.yaml"),
        r#"project: test
current:
  id: ms-001
  number: 1
  stage: 1
  stages:
    - id: 1
      scope: setup
      status: implementing
  gate_results: []
history: []
"#,
    )
    .unwrap();
    hlv::cmd::workflow::run(tmp.path(), false).unwrap();
}

#[test]
fn workflow_with_stages_implemented() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("milestones.yaml"),
        r#"project: test
current:
  id: ms-001
  number: 1
  stage: 1
  stages:
    - id: 1
      scope: setup
      status: implemented
  gate_results: []
history: []
"#,
    )
    .unwrap();
    hlv::cmd::workflow::run(tmp.path(), false).unwrap();
}

#[test]
fn workflow_with_stages_validating() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("milestones.yaml"),
        r#"project: test
current:
  id: ms-001
  number: 1
  stage: 1
  stages:
    - id: 1
      scope: setup
      status: validating
  gate_results: []
history: []
"#,
    )
    .unwrap();
    hlv::cmd::workflow::run(tmp.path(), false).unwrap();
}

#[test]
fn workflow_with_stages_validated() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("milestones.yaml"),
        r#"project: test
current:
  id: ms-001
  number: 1
  stage: 1
  stages:
    - id: 1
      scope: setup
      status: validated
  gate_results: []
history: []
"#,
    )
    .unwrap();
    hlv::cmd::workflow::run(tmp.path(), false).unwrap();
}

#[test]
fn workflow_all_stages_validated() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("milestones.yaml"),
        r#"project: test
current:
  id: ms-001
  number: 1
  stages:
    - id: 1
      scope: setup
      status: validated
    - id: 2
      scope: api
      status: validated
  gate_results: []
history: []
"#,
    )
    .unwrap();
    hlv::cmd::workflow::run(tmp.path(), false).unwrap();
}

#[test]
fn workflow_mixed_stages_no_active() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("milestones.yaml"),
        r#"project: test
current:
  id: ms-001
  number: 1
  stages:
    - id: 1
      scope: setup
      status: validated
    - id: 2
      scope: api
      status: pending
  gate_results: []
history: []
"#,
    )
    .unwrap();
    hlv::cmd::workflow::run(tmp.path(), false).unwrap();
}

#[test]
fn workflow_validated_stage_with_next_pending() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("milestones.yaml"),
        r#"project: test
current:
  id: ms-001
  number: 1
  stage: 1
  stages:
    - id: 1
      scope: setup
      status: validated
    - id: 2
      scope: api
      status: pending
  gate_results: []
history: []
"#,
    )
    .unwrap();
    hlv::cmd::workflow::run(tmp.path(), false).unwrap();
}

// ═══════════════════════════════════════════════════════
// cmd/check integration (run_checks via check::run)
// ═══════════════════════════════════════════════════════

#[test]
fn check_with_fixture_milestone_project() {
    // cmd/check::run calls process::exit, so we test the underlying check logic
    // by just verifying check_project_map doesn't crash on real fixture
    let root = Path::new("tests/fixtures/milestone-project");
    let diags = hlv::check::project_map::check_project_map(root);
    // project.yaml should be valid
    assert!(
        !diags.iter().any(|d| d.code == "PRJ-001"),
        "fixture project.yaml should parse"
    );
}

#[test]
fn check_collect_milestone_contracts_with_fixture() {
    // Test the full milestone-aware check pipeline indirectly
    let root = Path::new("tests/fixtures/milestone-project");
    let project = hlv::model::project::ProjectMap::load(&root.join("project.yaml")).unwrap();
    let milestones =
        hlv::model::milestone::MilestoneMap::load(&root.join("milestones.yaml")).unwrap();

    assert!(!project.project.is_empty());
    assert!(milestones.current.is_some());
}

// ═══════════════════════════════════════════════════════
// check/mod.rs — Diagnostic helpers
// ═══════════════════════════════════════════════════════

#[test]
fn diagnostic_with_file() {
    let d = check::Diagnostic::error("PRJ-001", "broken").with_file("project.yaml");
    assert_eq!(d.file.as_deref(), Some("project.yaml"));
    assert_eq!(d.code, "PRJ-001");
}

#[test]
fn diagnostic_print_does_not_panic() {
    // Just verify print() doesn't crash for all severity levels
    check::Diagnostic::error("E", "err").print();
    check::Diagnostic::warning("W", "warn").print();
    check::Diagnostic::info("I", "info").print();
    check::Diagnostic::error("E", "err")
        .with_file("f.yaml")
        .print();
}

#[test]
fn exit_code_zero_for_empty() {
    assert_eq!(check::exit_code(&[]), 0);
}

#[test]
fn exit_code_zero_for_warnings_only() {
    let diags = vec![check::Diagnostic::warning("W", "w")];
    assert_eq!(check::exit_code(&diags), 0);
}

#[test]
fn exit_code_one_for_errors() {
    let diags = vec![
        check::Diagnostic::warning("W", "w"),
        check::Diagnostic::error("E", "e"),
    ];
    assert_eq!(check::exit_code(&diags), 1);
}

#[test]
fn exit_code_zero_for_info_only() {
    let diags = vec![check::Diagnostic::info("I", "i")];
    assert_eq!(check::exit_code(&diags), 0);
}

// ═══════════════════════════════════════════════════════
// status with gates and history
// ═══════════════════════════════════════════════════════

#[test]
fn status_with_history() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_gates_project(root);
    fs::write(
        root.join("milestones.yaml"),
        r#"project: test
current:
  id: ms-002
  number: 2
  stages: []
  gate_results: []
history:
  - id: ms-001
    number: 1
    status: merged
"#,
    )
    .unwrap();
    hlv::cmd::status::run(root, false).unwrap();
}

#[test]
fn status_with_stages_and_contracts() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_gates_project(root);
    fs::write(
        root.join("milestones.yaml"),
        r#"project: test
current:
  id: ms-001
  number: 1
  stage: 1
  stages:
    - id: 1
      scope: setup
      status: implementing
      commit: abc1234def
  gate_results:
    - id: GATE-CONTRACT-001
      status: passed
      run_at: "2026-03-07T10:00:00Z"
history: []
"#,
    )
    .unwrap();
    // Create contracts dir with a file
    let contracts_dir = root.join("human/milestones/ms-001/contracts");
    fs::create_dir_all(&contracts_dir).unwrap();
    fs::write(
        contracts_dir.join("order.create.md"),
        "# order.create v1.0.0\n",
    )
    .unwrap();
    hlv::cmd::status::run(root, false).unwrap();
}

// ═══════════════════════════════════════════════════════
// CST-010 / CST-020 / CST-030: constraint checks
// ═══════════════════════════════════════════════════════

fn minimal_project_with_constraints(
    constraints: Vec<ConstraintEntry>,
) -> hlv::model::project::ProjectMap {
    use hlv::model::project::*;
    ProjectMap {
        schema_version: 1,
        project: "test".to_string(),
        spec: None,
        updated_at: None,
        status: ProjectStatus::Draft,
        last_skill: None,
        last_skill_result: None,
        paths: ProjectPaths {
            human: HumanPaths {
                glossary: "human/glossary.yaml".to_string(),
                constraints: "human/constraints/".to_string(),
                artifacts: Some("human/artifacts/".to_string()),
            },
            validation: ValidationPaths {
                gates_policy: "validation/gates-policy.yaml".to_string(),
                scenarios: "validation/scenarios/".to_string(),
                test_specs: Some("validation/test-specs/".to_string()),
                traceability: Some("human/traceability.yaml".to_string()),
                verify_report: None,
            },
            llm: LlmPaths {
                src: "llm/src/".to_string(),
                tests: None,
                map: None,
            },
        },
        glossary_types: vec![],
        constraints,
        validation: None,
        stack: None,
        git: Default::default(),
        features: Default::default(),
    }
}

#[test]
fn cst010_missing_constraint_file() {
    let tmp = TempDir::new().unwrap();
    let project = minimal_project_with_constraints(vec![ConstraintEntry {
        id: "constraints.security.global".to_string(),
        path: "human/constraints/nonexistent.yaml".to_string(),
        applies_to: None,
    }]);
    let diags = check_constraints(tmp.path(), &project);
    assert!(
        has_error(&diags, "CST-010"),
        "expected CST-010: {:?}",
        diags
    );
}

#[test]
fn cst010_unparseable_constraint_file() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("human/constraints")).unwrap();
    fs::write(
        tmp.path().join("human/constraints/bad.yaml"),
        "not: valid: yaml: [[[",
    )
    .unwrap();
    let project = minimal_project_with_constraints(vec![ConstraintEntry {
        id: "constraints.bad.global".to_string(),
        path: "human/constraints/bad.yaml".to_string(),
        applies_to: None,
    }]);
    let diags = check_constraints(tmp.path(), &project);
    assert!(
        has_error(&diags, "CST-010"),
        "expected CST-010 parse error: {:?}",
        diags
    );
}

#[test]
fn cst020_duplicate_rule_id() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("human/constraints")).unwrap();
    fs::write(
        tmp.path().join("human/constraints/dup.yaml"),
        r#"id: constraints.dup.global
version: "1.0.0"
rules:
  - id: same_rule
    severity: critical
    statement: "First rule"
  - id: same_rule
    severity: high
    statement: "Duplicate rule"
"#,
    )
    .unwrap();
    let project = minimal_project_with_constraints(vec![ConstraintEntry {
        id: "constraints.dup.global".to_string(),
        path: "human/constraints/dup.yaml".to_string(),
        applies_to: None,
    }]);
    let diags = check_constraints(tmp.path(), &project);
    assert!(
        has_error(&diags, "CST-020"),
        "expected CST-020 duplicate: {:?}",
        diags
    );
}

#[test]
fn cst030_invalid_severity() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("human/constraints")).unwrap();
    fs::write(
        tmp.path().join("human/constraints/bad_sev.yaml"),
        r#"id: constraints.badsev.global
version: "1.0.0"
rules:
  - id: rule_one
    severity: extreme
    statement: "Bad severity value"
"#,
    )
    .unwrap();
    let project = minimal_project_with_constraints(vec![ConstraintEntry {
        id: "constraints.badsev.global".to_string(),
        path: "human/constraints/bad_sev.yaml".to_string(),
        applies_to: None,
    }]);
    let diags = check_constraints(tmp.path(), &project);
    assert!(
        has_error(&diags, "CST-030"),
        "expected CST-030 invalid severity: {:?}",
        diags
    );
}

#[test]
fn cst_valid_constraint_no_errors() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("human/constraints")).unwrap();
    fs::write(
        tmp.path().join("human/constraints/good.yaml"),
        r#"id: constraints.good.global
version: "1.0.0"
rules:
  - id: rule_a
    severity: critical
    statement: "Good rule"
  - id: rule_b
    severity: low
    statement: "Another good rule"
"#,
    )
    .unwrap();
    let project = minimal_project_with_constraints(vec![ConstraintEntry {
        id: "constraints.good.global".to_string(),
        path: "human/constraints/good.yaml".to_string(),
        applies_to: None,
    }]);
    let diags = check_constraints(tmp.path(), &project);
    assert!(!has_any_error(&diags), "expected no errors: {:?}", diags);
}

// ═══════════════════════════════════════════════════════
// SEC-010: Security attention markers
// ═══════════════════════════════════════════════════════

#[test]
fn sec_markers_present_returns_sec010_info() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::create_dir_all(root.join("llm/src")).unwrap();
    fs::write(
        root.join("llm/src/handler.rs"),
        r#"
// @hlv:sec [INPUT_VALIDATION] — user email in query
fn validate_email() {}

// @hlv:sec [AUTH_BOUNDARY] — session check
fn check_auth() {}
"#,
    )
    .unwrap();

    let diags = check_sec_markers(root, "llm/src", true);
    assert!(
        has_info(&diags, "SEC-010"),
        "should have SEC-010 info: {:?}",
        diags
    );
    let sec010 = diags.iter().find(|d| d.code == "SEC-010").unwrap();
    assert!(sec010.message.contains("2 total"));
}

#[test]
fn sec_markers_no_markers_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::create_dir_all(root.join("llm/src")).unwrap();
    fs::write(root.join("llm/src/main.rs"), "fn main() {}").unwrap();

    let diags = check_sec_markers(root, "llm/src", true);
    assert!(diags.is_empty(), "no markers = no diags: {:?}", diags);
}

#[test]
fn sec_markers_invalid_category_warns() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::create_dir_all(root.join("llm/src")).unwrap();
    fs::write(
        root.join("llm/src/main.rs"),
        "// @hlv:sec [INVALID_CAT] — bad category",
    )
    .unwrap();

    let diags = check_sec_markers(root, "llm/src", true);
    assert!(
        has_warning(&diags, "SEC-011"),
        "should warn on invalid category: {:?}",
        diags
    );
}

#[test]
fn sec_markers_disabled_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::create_dir_all(root.join("llm/src")).unwrap();
    fs::write(
        root.join("llm/src/main.rs"),
        "// @hlv:sec [INPUT_VALIDATION] — should be ignored",
    )
    .unwrap();

    let diags = check_sec_markers(root, "llm/src", false);
    assert!(diags.is_empty(), "disabled = no diagnostics: {:?}", diags);
}
