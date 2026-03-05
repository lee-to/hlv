use hlv::model::contract_yaml::ContractYaml;
use hlv::model::glossary::Glossary;
use hlv::model::milestone::{ContractChangeAction, MilestoneMap, MilestoneStatus, StageStatus};
use hlv::model::policy::*;
use hlv::model::project::{ComponentType, DependencyType, ProjectMap};
use hlv::model::stage::StagePlan;
use hlv::model::traceability::TraceabilityMap;
use std::path::Path;

const FIXTURE: &str = "tests/fixtures/example-project";
const MS_FIXTURE: &str = "tests/fixtures/milestone-project";

#[test]
fn parse_project_yaml() {
    let p = ProjectMap::load(&Path::new(FIXTURE).join("project.yaml")).unwrap();
    assert_eq!(p.project, "commerce-checkout");
    assert_eq!(
        p.paths.validation.traceability.as_deref(),
        Some("human/traceability.yaml")
    );
    assert_eq!(p.constraints.len(), 2);
}

#[test]
fn parse_glossary() {
    let g = Glossary::load(&Path::new(FIXTURE).join("human/glossary.yaml")).unwrap();
    assert!(g.types.contains_key("UserId"));
    assert!(g.types.contains_key("Money"));
    assert!(g.enums.contains_key("OrderStatus"));
    assert_eq!(g.enums["OrderStatus"].len(), 4);
}

#[test]
fn parse_contract_yaml_create() {
    let c = ContractYaml::load(
        &Path::new(FIXTURE).join("human/milestones/001/contracts/order.create.yaml"),
    )
    .unwrap();
    assert_eq!(c.id, "order.create");
    assert_eq!(c.errors.len(), 3);
    assert_eq!(c.invariants.len(), 2);
}

#[test]
fn parse_contract_yaml_cancel() {
    let c = ContractYaml::load(
        &Path::new(FIXTURE).join("human/milestones/001/contracts/order.cancel.yaml"),
    )
    .unwrap();
    assert_eq!(c.id, "order.cancel");
    assert_eq!(c.errors.len(), 3);
}

#[test]
fn parse_gates_policy() {
    let p = GatesPolicy::load(&Path::new(FIXTURE).join("validation/gates-policy.yaml")).unwrap();
    assert_eq!(p.gates.len(), 7);
    assert!(p.gates.iter().all(|g| g.mandatory));
}

#[test]
fn parse_traceability_policy() {
    let p =
        TraceabilityPolicy::load(&Path::new(FIXTURE).join("validation/traceability-policy.yaml"))
            .unwrap();
    assert!(p.id_formats.is_some());
}

#[test]
fn parse_equivalence_policy() {
    let p = EquivalencePolicy::load(&Path::new(FIXTURE).join("validation/equivalence-policy.yaml"))
        .unwrap();
    assert_eq!(p.requirements.len(), 5);
}

#[test]
fn parse_ir_policy() {
    let p = IrPolicy::load(&Path::new(FIXTURE).join("validation/ir-policy.yaml")).unwrap();
    assert_eq!(p.compatibility_rules.len(), 5);
}

#[test]
fn parse_adversarial_guardrails() {
    let p = AdversarialGuardrails::load(
        &Path::new(FIXTURE).join("validation/adversarial-guardrails.yaml"),
    )
    .unwrap();
    assert_eq!(p.requirements.len(), 4);
}

#[test]
fn parse_traceability_map() {
    let t = TraceabilityMap::load(&Path::new(FIXTURE).join("human/traceability.yaml")).unwrap();
    assert_eq!(t.requirements.len(), 3);
    assert_eq!(t.mappings.len(), 3);
}

#[test]
fn parse_security_constraints() {
    let s = SecurityConstraints::load(&Path::new(FIXTURE).join("human/constraints/security.yaml"))
        .unwrap();
    assert_eq!(s.rules.len(), 6);
}

#[test]
fn parse_performance_constraints() {
    let p = PerformanceConstraints::load(
        &Path::new(FIXTURE).join("human/constraints/performance.yaml"),
    )
    .unwrap();
    assert_eq!(p.overrides.len(), 2);
}

#[test]
fn parse_project_yaml_stack() {
    let p = ProjectMap::load(&Path::new(FIXTURE).join("project.yaml")).unwrap();
    let stack = p.stack.expect("stack should be present");
    assert_eq!(stack.components.len(), 2);

    let backend = &stack.components[0];
    assert_eq!(backend.id, "backend");
    assert_eq!(backend.component_type, ComponentType::Service);
    assert_eq!(backend.languages, vec!["rust"]);
    assert_eq!(backend.dependencies.len(), 6);

    let axum = &backend.dependencies[0];
    assert_eq!(axum.name, "axum");
    assert_eq!(axum.dependency_type, DependencyType::Framework);
    assert!(axum.version.is_none());

    let pg = &backend.dependencies[4];
    assert_eq!(pg.name, "postgresql");
    assert_eq!(pg.dependency_type, DependencyType::Database);
    assert_eq!(pg.version.as_deref(), Some("16"));

    let migrations = &stack.components[1];
    assert_eq!(migrations.id, "migrations");
    assert_eq!(migrations.component_type, ComponentType::Script);
    assert_eq!(migrations.dependencies.len(), 1);
    assert_eq!(migrations.dependencies[0].name, "sqlx-cli");
    assert_eq!(
        migrations.dependencies[0].dependency_type,
        DependencyType::Tool
    );
}

// ── Milestone model tests ──────────────────────────────────────

#[test]
fn parse_milestones_yaml() {
    let m = MilestoneMap::load(&Path::new(MS_FIXTURE).join("milestones.yaml")).unwrap();
    assert_eq!(m.project, "commerce-checkout");

    let current = m.current.as_ref().expect("current milestone");
    assert_eq!(current.id, "001-checkout");
    assert_eq!(current.number, 1);
    assert_eq!(current.branch.as_deref(), Some("feature/checkout"));
    assert_eq!(current.stage, Some(1));
    assert_eq!(current.stages.len(), 2);

    assert_eq!(current.stages[0].id, 1);
    assert_eq!(current.stages[0].status, StageStatus::Implementing);
    assert_eq!(current.stages[1].id, 2);
    assert_eq!(current.stages[1].status, StageStatus::Pending);
}

#[test]
fn parse_milestones_verified_status() {
    let yaml = r#"
project: test
current:
  id: "001-init"
  number: 1
  stage: 1
  stages:
    - id: 1
      scope: "setup"
      status: verified
history: []
"#;
    let m: MilestoneMap = serde_yaml::from_str(yaml).unwrap();
    let current = m.current.unwrap();
    assert_eq!(current.stages[0].status, StageStatus::Verified);
}

#[test]
fn parse_milestones_yaml_with_history() {
    let yaml = r#"
project: test
current:
  id: "002-fix"
  number: 2
  stage: 1
  stages:
    - id: 1
      scope: "Fix race condition"
      status: implementing
history:
  - id: "001-initial"
    number: 1
    status: merged
    contracts:
      - name: order.create
        action: created
      - name: order.cancel
        action: created
    branch: feature/initial
    merged_at: "2026-03-01"
"#;
    let m: MilestoneMap = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(m.history.len(), 1);
    assert_eq!(m.history[0].status, MilestoneStatus::Merged);
    assert_eq!(m.history[0].contracts.len(), 2);
    assert_eq!(
        m.history[0].contracts[0].action,
        ContractChangeAction::Created
    );
}

#[test]
fn milestone_next_number() {
    let yaml = r#"
project: test
current:
  id: "003-payment"
  number: 3
  stages: []
history:
  - id: "001-initial"
    number: 1
    status: merged
  - id: "002-fix"
    number: 2
    status: merged
"#;
    let m: MilestoneMap = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(m.next_number(), 4);
}

#[test]
fn milestone_next_number_no_current() {
    let yaml = r#"
project: test
history:
  - id: "001-initial"
    number: 1
    status: merged
"#;
    let m: MilestoneMap = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(m.next_number(), 2);
}

#[test]
fn milestone_resolve_contract_from_history() {
    let yaml = r#"
project: test
history:
  - id: "001-initial"
    number: 1
    status: merged
    contracts:
      - name: order.create
        action: created
  - id: "002-fix"
    number: 2
    status: merged
    contracts:
      - name: order.create
        action: modified
"#;
    let m: MilestoneMap = serde_yaml::from_str(yaml).unwrap();
    let resolved = m.resolve_contract("order.create").unwrap();
    // Should resolve to the most recent (002-fix)
    assert_eq!(resolved.milestone_id, "002-fix");
    assert_eq!(resolved.milestone_number, 2);
}

// ── Stage plan tests ──────────────────────────────────────

#[test]
fn parse_stage_file() {
    let stage =
        StagePlan::load(&Path::new(MS_FIXTURE).join("human/milestones/001-checkout/stage_1.md"))
            .unwrap();
    assert_eq!(stage.id, 1);
    assert_eq!(stage.name, "Foundation");
    assert_eq!(stage.budget.as_deref(), Some("~25K"));
    assert_eq!(stage.contracts, vec!["order.create", "order.cancel"]);
    assert_eq!(stage.tasks.len(), 4);
    assert_eq!(stage.tasks[0].id, "TASK-001");
    assert_eq!(stage.tasks[3].id, "TASK-004");
    assert_eq!(stage.tasks[3].depends_on, vec!["TASK-002", "TASK-003"]);
}

#[test]
fn parse_stage_2_file() {
    let stage =
        StagePlan::load(&Path::new(MS_FIXTURE).join("human/milestones/001-checkout/stage_2.md"))
            .unwrap();
    assert_eq!(stage.id, 2);
    assert_eq!(stage.name, "Integration + Observability");
    assert_eq!(stage.tasks.len(), 2);
    assert!(stage.tasks[0].depends_on.is_empty());
}

// ═══════════════════════════════════════════════════════
// Integration: load → modify → save → reload
// ═══════════════════════════════════════════════════════

#[test]
fn project_map_modify_save_reload() {
    use hlv::model::project::ConstraintEntry;
    use tempfile::TempDir;

    let original = ProjectMap::load(&Path::new(FIXTURE).join("project.yaml")).unwrap();
    let tmp = TempDir::new().unwrap();
    let out = tmp.path().join("project.yaml");

    // Save original, reload, verify
    original.save(&out).unwrap();
    let mut reloaded = ProjectMap::load(&out).unwrap();
    assert_eq!(reloaded.project, original.project);
    assert_eq!(reloaded.constraints.len(), original.constraints.len());

    // Modify: add a constraint
    reloaded
        .add_constraint(ConstraintEntry {
            id: "constraints.test.global".to_string(),
            path: "human/constraints/test.yaml".to_string(),
            applies_to: Some("global".to_string()),
        })
        .unwrap();
    reloaded.save(&out).unwrap();

    // Reload and verify modification persisted
    let final_map = ProjectMap::load(&out).unwrap();
    assert_eq!(final_map.constraints.len(), original.constraints.len() + 1);
    assert!(final_map
        .constraints
        .iter()
        .any(|c| c.id == "constraints.test.global"));
}

#[test]
fn llm_map_modify_save_reload() {
    use hlv::model::llm_map::{LlmMap, MapEntry};
    use tempfile::TempDir;

    let original = LlmMap::load(&Path::new(FIXTURE).join("llm/map.yaml")).unwrap();
    let tmp = TempDir::new().unwrap();
    let out = tmp.path().join("map.yaml");

    original.save(&out).unwrap();
    let mut reloaded = LlmMap::load(&out).unwrap();
    assert_eq!(reloaded.entries.len(), original.entries.len());

    // Add entry
    reloaded
        .add_entry(MapEntry {
            path: "human/constraints/test.yaml".to_string(),
            kind: hlv::model::llm_map::MapEntryKind::File,
            layer: "human".to_string(),
            description: "Test constraint".to_string(),
        })
        .unwrap();
    reloaded.save(&out).unwrap();

    let final_map = LlmMap::load(&out).unwrap();
    assert_eq!(final_map.entries.len(), original.entries.len() + 1);
    assert!(final_map
        .entries
        .iter()
        .any(|e| e.path == "human/constraints/test.yaml"));
}
