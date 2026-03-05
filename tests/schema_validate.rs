use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;
use tempfile::TempDir;

use hlv::model::glossary::Glossary;
use hlv::model::milestone::MilestoneMap;
use hlv::model::policy::{
    AdversarialGuardrails, ConstraintFile, EquivalencePolicy, GatesPolicy, IrPolicy,
    PerformanceConstraints, SecurityConstraints, TraceabilityPolicy,
};
use hlv::model::project::ProjectMap;
use hlv::model::traceability::TraceabilityMap;

const FIXTURE: &str = "tests/fixtures/example-project";
const MS_FIXTURE: &str = "tests/fixtures/milestone-project";
const SCHEMA_DIR: &str = "schema";

const FIXTURE_CASES: &[(&str, &str, &str)] = &[
    // --- example-project ---
    (FIXTURE, "project-schema.json", "project.yaml"),
    (FIXTURE, "glossary-schema.json", "human/glossary.yaml"),
    (
        FIXTURE,
        "contract-schema.json",
        "human/milestones/001/contracts/order.create.yaml",
    ),
    (
        FIXTURE,
        "contract-schema.json",
        "human/milestones/001/contracts/order.cancel.yaml",
    ),
    (
        FIXTURE,
        "security-constraints-schema.json",
        "human/constraints/security.yaml",
    ),
    (
        FIXTURE,
        "performance-constraints-schema.json",
        "human/constraints/performance.yaml",
    ),
    (
        FIXTURE,
        "traceability-schema.json",
        "human/traceability.yaml",
    ),
    (FIXTURE, "llm-map-schema.json", "llm/map.yaml"),
    (
        FIXTURE,
        "gates-policy-schema.json",
        "validation/gates-policy.yaml",
    ),
    (
        FIXTURE,
        "equivalence-policy-schema.json",
        "validation/equivalence-policy.yaml",
    ),
    (
        FIXTURE,
        "traceability-policy-schema.json",
        "validation/traceability-policy.yaml",
    ),
    (
        FIXTURE,
        "ir-policy-schema.json",
        "validation/ir-policy.yaml",
    ),
    (
        FIXTURE,
        "adversarial-guardrails-schema.json",
        "validation/adversarial-guardrails.yaml",
    ),
    // --- constraint-schema (generic, same fixture as security) ---
    (
        FIXTURE,
        "constraint-schema.json",
        "human/constraints/security.yaml",
    ),
    // --- milestone-project (full set) ---
    (MS_FIXTURE, "project-schema.json", "project.yaml"),
    (MS_FIXTURE, "milestones-schema.json", "milestones.yaml"),
    (MS_FIXTURE, "glossary-schema.json", "human/glossary.yaml"),
    (
        MS_FIXTURE,
        "security-constraints-schema.json",
        "human/constraints/security.yaml",
    ),
    (
        MS_FIXTURE,
        "performance-constraints-schema.json",
        "human/constraints/performance.yaml",
    ),
    (
        MS_FIXTURE,
        "traceability-schema.json",
        "human/traceability.yaml",
    ),
    (MS_FIXTURE, "llm-map-schema.json", "llm/map.yaml"),
    (
        MS_FIXTURE,
        "contract-schema.json",
        "human/milestones/001-checkout/contracts/order.create.yaml",
    ),
    (
        MS_FIXTURE,
        "gates-policy-schema.json",
        "validation/gates-policy.yaml",
    ),
    (
        MS_FIXTURE,
        "equivalence-policy-schema.json",
        "validation/equivalence-policy.yaml",
    ),
    (
        MS_FIXTURE,
        "traceability-policy-schema.json",
        "validation/traceability-policy.yaml",
    ),
    (
        MS_FIXTURE,
        "ir-policy-schema.json",
        "validation/ir-policy.yaml",
    ),
    (
        MS_FIXTURE,
        "adversarial-guardrails-schema.json",
        "validation/adversarial-guardrails.yaml",
    ),
];

const INIT_GENERATED_CASES: &[(&str, &str)] = &[
    ("project-schema.json", "project.yaml"),
    ("milestones-schema.json", "milestones.yaml"),
    ("glossary-schema.json", "human/glossary.yaml"),
    (
        "security-constraints-schema.json",
        "human/constraints/security.yaml",
    ),
    (
        "performance-constraints-schema.json",
        "human/constraints/performance.yaml",
    ),
    (
        "security-constraints-schema.json",
        "human/constraints/observability.yaml",
    ),
    ("traceability-schema.json", "human/traceability.yaml"),
    ("llm-map-schema.json", "llm/map.yaml"),
    ("gates-policy-schema.json", "validation/gates-policy.yaml"),
    (
        "equivalence-policy-schema.json",
        "validation/equivalence-policy.yaml",
    ),
    (
        "traceability-policy-schema.json",
        "validation/traceability-policy.yaml",
    ),
    ("ir-policy-schema.json", "validation/ir-policy.yaml"),
    (
        "adversarial-guardrails-schema.json",
        "validation/adversarial-guardrails.yaml",
    ),
];

fn validate_in(root: &Path, schema_name: &str, yaml_rel: &str) {
    let schema_path = Path::new(SCHEMA_DIR).join(schema_name);
    let yaml_path = root.join(yaml_rel);

    let schema_str = std::fs::read_to_string(&schema_path)
        .unwrap_or_else(|e| panic!("read schema {}: {e}", schema_path.display()));
    let schema: Value = serde_json::from_str(&schema_str)
        .unwrap_or_else(|e| panic!("parse schema {}: {e}", schema_path.display()));

    let yaml_str = std::fs::read_to_string(&yaml_path)
        .unwrap_or_else(|e| panic!("read yaml {}: {e}", yaml_path.display()));
    let yaml_val: serde_yaml::Value = serde_yaml::from_str(&yaml_str)
        .unwrap_or_else(|e| panic!("parse yaml {}: {e}", yaml_path.display()));

    let json_val: Value = serde_json::to_value(yaml_val)
        .unwrap_or_else(|e| panic!("yaml->json {}: {e}", yaml_path.display()));

    let validator = jsonschema::validator_for(&schema)
        .unwrap_or_else(|e| panic!("compile schema {}: {e}", schema_path.display()));

    let errors: Vec<String> = validator
        .iter_errors(&json_val)
        .map(|e| format!("  {}: {}", e.instance_path, e))
        .collect();

    if !errors.is_empty() {
        panic!(
            "Schema validation failed for {} against {schema_name}:\n{}",
            yaml_path.display(),
            errors.join("\n")
        );
    }
}

fn validate_fixture(schema_name: &str, yaml_rel: &str) {
    validate_in(Path::new(FIXTURE), schema_name, yaml_rel);
}

#[test]
fn all_fixture_yaml_match_schemas() {
    for (fixture, schema, yaml_rel) in FIXTURE_CASES {
        validate_in(Path::new(fixture), schema, yaml_rel);
    }
}

#[test]
fn schema_dir_is_fully_covered_by_tests() {
    let mut actual: BTreeSet<String> = BTreeSet::new();
    for entry in std::fs::read_dir(SCHEMA_DIR).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            actual.insert(name);
        }
    }

    let covered: BTreeSet<String> = FIXTURE_CASES
        .iter()
        .map(|(_, schema, _)| (*schema).to_string())
        .collect();

    assert_eq!(
        actual, covered,
        "every schema/*.json must be covered by at least one fixture validation case"
    );
}

fn assert_generated_yaml_is_schema_valid(profile: &str) {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().to_str().unwrap();

    hlv::cmd::init::run_with_milestone(
        path,
        Some("schema-check"),
        Some("qa"),
        Some("claude"),
        Some("init"),
        Some(profile),
    )
    .unwrap();

    for (schema, yaml_rel) in INIT_GENERATED_CASES {
        validate_in(tmp.path(), schema, yaml_rel);
    }

    // Exercise gates-policy writer path (enabled/command/cwd fields).
    hlv::cmd::gates::run_set_command(tmp.path(), "GATE-CONTRACT-001", "true").unwrap();
    hlv::cmd::gates::run_set_cwd(tmp.path(), "GATE-CONTRACT-001", "llm").unwrap();
    hlv::cmd::gates::run_disable(tmp.path(), "GATE-CONTRACT-001").unwrap();
    hlv::cmd::gates::run_enable(tmp.path(), "GATE-CONTRACT-001").unwrap();
    validate_in(
        tmp.path(),
        "gates-policy-schema.json",
        "validation/gates-policy.yaml",
    );

    // Exercise milestones writer path with non-empty gate_results.
    let (passed, failed, skipped) = hlv::cmd::gates::run_gate_commands(tmp.path(), None).unwrap();
    assert_eq!((passed, failed, skipped), (1, 0, 0));
    validate_in(tmp.path(), "milestones-schema.json", "milestones.yaml");

    // Exercise milestones writer path when current milestone is removed.
    hlv::cmd::milestone::run_abort(tmp.path()).unwrap();
    validate_in(tmp.path(), "milestones-schema.json", "milestones.yaml");
}

#[test]
fn generated_yaml_matches_schema_minimal_profile() {
    assert_generated_yaml_is_schema_valid("minimal");
}

#[test]
fn generated_yaml_matches_schema_standard_profile() {
    assert_generated_yaml_is_schema_valid("standard");
}

#[test]
fn generated_yaml_matches_schema_full_profile() {
    assert_generated_yaml_is_schema_valid("full");
}

#[test]
fn fixture_project_yaml_matches_project_schema() {
    validate_fixture("project-schema.json", "project.yaml");
}

// ═══════════════════════════════════════════════════════
// Rust struct ↔ YAML roundtrip — catches dead/missing fields
// ═══════════════════════════════════════════════════════
// If a schema field is removed but Rust still has it → fixture YAML won't have it → roundtrip differs.
// If a schema field is added → fixture must be updated → test fails.
// If Rust struct has a typo → deserialization fails.

/// Load YAML into Rust struct, serialize back, re-validate against schema.
/// This catches Rust struct fields that don't match the schema.
fn roundtrip_validate(root: &Path, schema_name: &str, yaml_rel: &str, loader: fn(&Path) -> String) {
    let yaml_path = root.join(yaml_rel);

    // 1. Load via Rust struct and serialize back
    let reserialized = loader(&yaml_path);

    // 2. Validate reserialized YAML against schema
    let schema_path = Path::new(SCHEMA_DIR).join(schema_name);
    let schema_str = std::fs::read_to_string(&schema_path).unwrap();
    let schema: Value = serde_json::from_str(&schema_str).unwrap();

    let yaml_val: serde_yaml::Value = serde_yaml::from_str(&reserialized).unwrap_or_else(|e| {
        panic!(
            "reserialized YAML for {} is invalid: {e}\n---\n{reserialized}",
            yaml_rel
        )
    });
    let json_val: Value = serde_json::to_value(yaml_val).unwrap();

    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<String> = validator
        .iter_errors(&json_val)
        .map(|e| format!("  {}: {}", e.instance_path, e))
        .collect();

    if !errors.is_empty() {
        panic!(
            "Roundtrip schema validation failed for {} (struct→YAML→schema):\n{}\n---reserialized---\n{}",
            yaml_rel,
            errors.join("\n"),
            reserialized
        );
    }
}

fn load_project(path: &Path) -> String {
    let m = ProjectMap::load(path).unwrap();
    serde_yaml::to_string(&m).unwrap()
}

fn load_milestones(path: &Path) -> String {
    let m = MilestoneMap::load(path).unwrap();
    serde_yaml::to_string(&m).unwrap()
}

fn load_glossary(path: &Path) -> String {
    let m = Glossary::load(path).unwrap();
    serde_yaml::to_string(&m).unwrap()
}

fn load_gates_policy(path: &Path) -> String {
    let m = GatesPolicy::load(path).unwrap();
    serde_yaml::to_string(&m).unwrap()
}

fn load_traceability(path: &Path) -> String {
    let m = TraceabilityMap::load(path).unwrap();
    serde_yaml::to_string(&m).unwrap()
}

fn load_security(path: &Path) -> String {
    let m = SecurityConstraints::load(path).unwrap();
    serde_yaml::to_string(&m).unwrap()
}

fn load_performance(path: &Path) -> String {
    let m = PerformanceConstraints::load(path).unwrap();
    serde_yaml::to_string(&m).unwrap()
}

fn load_equivalence(path: &Path) -> String {
    let m = EquivalencePolicy::load(path).unwrap();
    serde_yaml::to_string(&m).unwrap()
}

fn load_traceability_policy(path: &Path) -> String {
    let m = TraceabilityPolicy::load(path).unwrap();
    serde_yaml::to_string(&m).unwrap()
}

fn load_ir_policy(path: &Path) -> String {
    let m = IrPolicy::load(path).unwrap();
    serde_yaml::to_string(&m).unwrap()
}

fn load_adversarial(path: &Path) -> String {
    let m = AdversarialGuardrails::load(path).unwrap();
    serde_yaml::to_string(&m).unwrap()
}

fn load_constraint_file(path: &Path) -> String {
    let m = ConstraintFile::load(path).unwrap();
    serde_yaml::to_string(&m).unwrap()
}

fn load_contract_yaml(path: &Path) -> String {
    let m = hlv::model::contract_yaml::ContractYaml::load(path).unwrap();
    serde_yaml::to_string(&m).unwrap()
}

fn load_llm_map(path: &Path) -> String {
    let m = hlv::model::llm_map::LlmMap::load(path).unwrap();
    serde_yaml::to_string(&m).unwrap()
}

/// Roundtrip cases: (fixture_root, schema, yaml_rel, loader)
const ROUNDTRIP_CASES: &[(&str, &str, &str, fn(&Path) -> String)] = &[
    (FIXTURE, "project-schema.json", "project.yaml", load_project),
    (
        FIXTURE,
        "glossary-schema.json",
        "human/glossary.yaml",
        load_glossary,
    ),
    (
        FIXTURE,
        "gates-policy-schema.json",
        "validation/gates-policy.yaml",
        load_gates_policy,
    ),
    (
        FIXTURE,
        "traceability-schema.json",
        "human/traceability.yaml",
        load_traceability,
    ),
    (
        FIXTURE,
        "security-constraints-schema.json",
        "human/constraints/security.yaml",
        load_security,
    ),
    (
        FIXTURE,
        "performance-constraints-schema.json",
        "human/constraints/performance.yaml",
        load_performance,
    ),
    (
        FIXTURE,
        "equivalence-policy-schema.json",
        "validation/equivalence-policy.yaml",
        load_equivalence,
    ),
    (
        FIXTURE,
        "traceability-policy-schema.json",
        "validation/traceability-policy.yaml",
        load_traceability_policy,
    ),
    (
        FIXTURE,
        "ir-policy-schema.json",
        "validation/ir-policy.yaml",
        load_ir_policy,
    ),
    (
        FIXTURE,
        "adversarial-guardrails-schema.json",
        "validation/adversarial-guardrails.yaml",
        load_adversarial,
    ),
    (
        FIXTURE,
        "contract-schema.json",
        "human/milestones/001/contracts/order.create.yaml",
        load_contract_yaml,
    ),
    (FIXTURE, "llm-map-schema.json", "llm/map.yaml", load_llm_map),
    (
        FIXTURE,
        "constraint-schema.json",
        "human/constraints/security.yaml",
        load_constraint_file,
    ),
    (
        MS_FIXTURE,
        "project-schema.json",
        "project.yaml",
        load_project,
    ),
    (
        MS_FIXTURE,
        "milestones-schema.json",
        "milestones.yaml",
        load_milestones,
    ),
    (
        MS_FIXTURE,
        "glossary-schema.json",
        "human/glossary.yaml",
        load_glossary,
    ),
    (
        MS_FIXTURE,
        "gates-policy-schema.json",
        "validation/gates-policy.yaml",
        load_gates_policy,
    ),
    (
        MS_FIXTURE,
        "traceability-schema.json",
        "human/traceability.yaml",
        load_traceability,
    ),
    (
        MS_FIXTURE,
        "security-constraints-schema.json",
        "human/constraints/security.yaml",
        load_security,
    ),
    (
        MS_FIXTURE,
        "performance-constraints-schema.json",
        "human/constraints/performance.yaml",
        load_performance,
    ),
    (
        MS_FIXTURE,
        "contract-schema.json",
        "human/milestones/001-checkout/contracts/order.create.yaml",
        load_contract_yaml,
    ),
    (
        MS_FIXTURE,
        "llm-map-schema.json",
        "llm/map.yaml",
        load_llm_map,
    ),
];

#[test]
fn roundtrip_all_fixtures_valid_after_rust_parse() {
    for (fixture_root, schema, yaml_rel, loader) in ROUNDTRIP_CASES {
        roundtrip_validate(Path::new(fixture_root), schema, yaml_rel, *loader);
    }
}

#[test]
fn milestone_project_all_yaml_valid() {
    // Validate every YAML in milestone-project against its schema
    let ms_cases: &[(&str, &str)] = &[
        ("project-schema.json", "project.yaml"),
        ("milestones-schema.json", "milestones.yaml"),
        ("glossary-schema.json", "human/glossary.yaml"),
        (
            "security-constraints-schema.json",
            "human/constraints/security.yaml",
        ),
        (
            "performance-constraints-schema.json",
            "human/constraints/performance.yaml",
        ),
        ("traceability-schema.json", "human/traceability.yaml"),
        ("llm-map-schema.json", "llm/map.yaml"),
        (
            "contract-schema.json",
            "human/milestones/001-checkout/contracts/order.create.yaml",
        ),
        ("gates-policy-schema.json", "validation/gates-policy.yaml"),
        (
            "equivalence-policy-schema.json",
            "validation/equivalence-policy.yaml",
        ),
        (
            "traceability-policy-schema.json",
            "validation/traceability-policy.yaml",
        ),
        ("ir-policy-schema.json", "validation/ir-policy.yaml"),
        (
            "adversarial-guardrails-schema.json",
            "validation/adversarial-guardrails.yaml",
        ),
    ];
    for (schema, yaml_rel) in ms_cases {
        validate_in(Path::new(MS_FIXTURE), schema, yaml_rel);
    }
}

#[test]
fn dead_field_detection_works() {
    // Verify that additionalProperties: false in schemas catches unknown fields.
    // Use project-schema which has additionalProperties: false at root.
    let schema_str =
        std::fs::read_to_string(Path::new(SCHEMA_DIR).join("project-schema.json")).unwrap();
    let schema: Value = serde_json::from_str(&schema_str).unwrap();

    // Create a project.yaml with an extra unknown field
    let yaml_with_dead_field = r#"
schema_version: 1
project: test
status: draft
dead_field_that_should_not_exist: true
paths:
  human:
    glossary: human/glossary.yaml
    constraints: human/constraints/
  validation:
    gates_policy: validation/gates-policy.yaml
    scenarios: validation/scenarios/
  llm:
    src: llm/src/
"#;

    let yaml_val: serde_yaml::Value = serde_yaml::from_str(yaml_with_dead_field).unwrap();
    let json_val: Value = serde_json::to_value(yaml_val).unwrap();

    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<String> = validator
        .iter_errors(&json_val)
        .map(|e| format!("{}", e))
        .collect();

    assert!(
        errors
            .iter()
            .any(|e| e.contains("dead_field_that_should_not_exist")),
        "additionalProperties: false should catch dead fields, errors: {:?}",
        errors
    );
}
