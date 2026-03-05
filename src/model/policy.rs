use serde::{Deserialize, Serialize};

// ─── Gates Policy ───────────────────────────────────────────────

/// validation/gates-policy.yaml
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GatesPolicy {
    pub version: String,
    pub policy_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_policy: Option<ReleasePolicy>,
    #[serde(default)]
    pub gates: Vec<Gate>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ReleasePolicy {
    #[serde(default)]
    pub require_all_mandatory: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interpretation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flaky_policy: Option<FlakyPolicy>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct FlakyPolicy {
    #[serde(default)]
    pub quarantine_required: bool,
    #[serde(default)]
    pub block_release_for_p0: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    pub id: String,
    #[serde(rename = "type")]
    pub gate_type: String,
    #[serde(default)]
    pub mandatory: bool,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pass_criteria: Option<serde_yaml::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Working directory for command, relative to project root. Defaults to project root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
}

fn default_enabled() -> bool {
    true
}

impl GatesPolicy {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_yaml::from_str(&content)?)
    }

    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let yaml = serde_yaml::to_string(self)?;
        let content = format!(
            "# yaml-language-server: $schema=../schema/gates-policy-schema.json\n{}",
            yaml
        );
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn find_gate_mut(&mut self, id: &str) -> Option<&mut Gate> {
        self.gates.iter_mut().find(|g| g.id == id)
    }

    pub fn add_gate(&mut self, gate: Gate) -> anyhow::Result<()> {
        if self.gates.iter().any(|g| g.id == gate.id) {
            anyhow::bail!("Gate '{}' already exists", gate.id);
        }
        self.gates.push(gate);
        Ok(())
    }

    pub fn remove_gate(&mut self, id: &str) -> anyhow::Result<Gate> {
        let pos = self
            .gates
            .iter()
            .position(|g| g.id == id)
            .ok_or_else(|| anyhow::anyhow!("Gate '{}' not found", id))?;
        Ok(self.gates.remove(pos))
    }
}

// ─── Traceability Policy ────────────────────────────────────────

/// validation/traceability-policy.yaml
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TraceabilityPolicy {
    pub version: String,
    pub policy_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id_formats: Option<IdFormats>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_requirements: Option<GraphRequirements>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct IdFormats {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct GraphRequirements {
    #[serde(default)]
    pub required_paths: Vec<String>,
    #[serde(default)]
    pub checks: Vec<TraceCheck>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct TraceCheck {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub must: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
}

impl TraceabilityPolicy {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_yaml::from_str(&content)?)
    }
}

// ─── Equivalence Policy ─────────────────────────────────────────

/// validation/equivalence-policy.yaml
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EquivalencePolicy {
    pub version: String,
    pub policy_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<EquivalenceScope>,
    #[serde(default)]
    pub requirements: Vec<EquivalenceRequirement>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EquivalenceScope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applies_to: Option<String>,
    #[serde(default)]
    pub required_for_regeneration: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EquivalenceRequirement {
    pub id: String,
    pub rule: String,
    #[serde(default)]
    pub must: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalize_fields: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_approaches: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numeric: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub categorical: Option<String>,
}

impl EquivalencePolicy {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_yaml::from_str(&content)?)
    }
}

// ─── IR Policy ──────────────────────────────────────────────────

/// validation/ir-policy.yaml
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IrPolicy {
    pub version: String,
    pub policy_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub compatibility_rules: Vec<IrRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_fields: Option<IrRequiredFields>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct IrRule {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub must: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub may: Option<bool>,
    pub rule: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct IrRequiredFields {
    #[serde(default)]
    pub contract_ir: Vec<String>,
    #[serde(default)]
    pub test_ir: Vec<String>,
}

impl IrPolicy {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_yaml::from_str(&content)?)
    }
}

// ─── Adversarial Guardrails ─────────────────────────────────────

/// validation/adversarial-guardrails.yaml
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdversarialGuardrails {
    pub version: String,
    pub policy_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub requirements: Vec<AdversarialRequirement>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct AdversarialRequirement {
    pub id: String,
    #[serde(default)]
    pub must: bool,
    pub rule: String,
}

impl AdversarialGuardrails {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_yaml::from_str(&content)?)
    }
}

// ─── Generic Rule-Based Constraint ──────────────────────────────

/// Generic rule-based constraint file (security, observability, compliance, etc.)
/// Compatible with SecurityConstraints — same fields.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConstraintFile {
    pub id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    #[serde(default)]
    pub rules: Vec<ConstraintRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exceptions: Option<ExceptionPolicy>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ConstraintRule {
    pub id: String,
    pub severity: String,
    pub statement: String,
    #[serde(default)]
    pub enforcement: Vec<String>,
}

impl ConstraintFile {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_yaml::from_str(&content)?)
    }

    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let yaml = serde_yaml::to_string(self)?;
        let content = format!(
            "# yaml-language-server: $schema=../../schema/constraint-schema.json\n{}",
            yaml
        );
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn add_rule(&mut self, rule: ConstraintRule) -> anyhow::Result<()> {
        if self.rules.iter().any(|r| r.id == rule.id) {
            anyhow::bail!(
                "Rule '{}' already exists in constraint '{}'",
                rule.id,
                self.id
            );
        }
        self.rules.push(rule);
        Ok(())
    }

    pub fn remove_rule(&mut self, rule_id: &str) -> anyhow::Result<ConstraintRule> {
        let pos = self
            .rules
            .iter()
            .position(|r| r.id == rule_id)
            .ok_or_else(|| {
                anyhow::anyhow!("Rule '{}' not found in constraint '{}'", rule_id, self.id)
            })?;
        Ok(self.rules.remove(pos))
    }
}

// ─── Security Constraints ───────────────────────────────────────

/// human/constraints/security.yaml
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityConstraints {
    pub id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    #[serde(default)]
    pub rules: Vec<SecurityConstraintRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exceptions: Option<ExceptionPolicy>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SecurityConstraintRule {
    pub id: String,
    pub severity: String,
    pub statement: String,
    #[serde(default)]
    pub enforcement: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ExceptionPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_exception_days: Option<u32>,
}

impl SecurityConstraints {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_yaml::from_str(&content)?)
    }
}

// ─── Performance Constraints ────────────────────────────────────

/// human/constraints/performance.yaml
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceConstraints {
    pub id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defaults: Option<PerfDefaults>,
    #[serde(default)]
    pub overrides: Vec<PerfOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<PerfValidation>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct PerfDefaults {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_p95_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_p99_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_rate_max_percent: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub availability_slo: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_max_percent: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_max_mb: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct PerfOverride {
    pub contract_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_p99_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub throughput_rps_min: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct PerfValidation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warmup_seconds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_window_seconds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percentile_method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fail_on_budget_exceed: Option<bool>,
}

impl PerformanceConstraints {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_yaml::from_str(&content)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gates_policy_load_save_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gates.yaml");

        let policy = GatesPolicy {
            version: "1.0.0".to_string(),
            policy_id: "TEST".to_string(),
            description: Some("Test policy".to_string()),
            release_policy: Some(ReleasePolicy {
                require_all_mandatory: true,
                interpretation: None,
                flaky_policy: Some(FlakyPolicy {
                    quarantine_required: true,
                    block_release_for_p0: true,
                }),
            }),
            gates: vec![Gate {
                id: "GATE-001".to_string(),
                gate_type: "contract_tests".to_string(),
                mandatory: true,
                enabled: true,
                pass_criteria: None,
                command: Some("cargo test".to_string()),
                cwd: None,
                tools: None,
            }],
        };

        policy.save(&path).unwrap();
        let loaded = GatesPolicy::load(&path).unwrap();
        assert_eq!(loaded.policy_id, "TEST");
        assert_eq!(loaded.gates.len(), 1);
        assert_eq!(loaded.gates[0].id, "GATE-001");
        assert!(loaded.gates[0].enabled);
        assert!(loaded.gates[0].mandatory);
        assert_eq!(loaded.gates[0].command.as_deref(), Some("cargo test"));
    }

    #[test]
    fn gates_policy_find_gate_mut() {
        let mut policy = GatesPolicy {
            version: "1.0.0".to_string(),
            policy_id: "TEST".to_string(),
            description: None,
            release_policy: None,
            gates: vec![
                Gate {
                    id: "GATE-001".to_string(),
                    gate_type: "unit".to_string(),
                    mandatory: true,
                    enabled: true,
                    pass_criteria: None,
                    command: None,
                    cwd: None,
                    tools: None,
                },
                Gate {
                    id: "GATE-002".to_string(),
                    gate_type: "integration".to_string(),
                    mandatory: false,
                    enabled: false,
                    pass_criteria: None,
                    command: None,
                    cwd: None,
                    tools: None,
                },
            ],
        };

        // Found
        let gate = policy.find_gate_mut("GATE-002");
        assert!(gate.is_some());
        let g = gate.unwrap();
        g.enabled = true;
        assert!(policy.gates[1].enabled);

        // Not found
        assert!(policy.find_gate_mut("GATE-999").is_none());
    }

    #[test]
    fn gates_policy_load_missing_file() {
        let result = GatesPolicy::load(std::path::Path::new("/nonexistent/gates.yaml"));
        assert!(result.is_err());
    }

    #[test]
    fn gates_policy_default_enabled() {
        let yaml = r#"
version: "1.0.0"
policy_id: TEST
gates:
  - id: G1
    type: unit
    mandatory: true
"#;
        let policy: GatesPolicy = serde_yaml::from_str(yaml).unwrap();
        assert!(policy.gates[0].enabled, "enabled should default to true");
    }

    #[test]
    fn traceability_policy_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trace.yaml");
        std::fs::write(
            &path,
            r#"
version: "1.0.0"
policy_id: TRACE
id_formats:
  requirement: "^REQ-"
  contract: "^CTR-"
graph_requirements:
  required_paths:
    - "requirement -> contract"
  checks:
    - id: TRACE-001
      name: no_dangling
      must: true
"#,
        )
        .unwrap();

        let policy = TraceabilityPolicy::load(&path).unwrap();
        assert_eq!(policy.policy_id, "TRACE");
        assert!(policy.id_formats.is_some());
        let gr = policy.graph_requirements.unwrap();
        assert_eq!(gr.checks.len(), 1);
        assert!(gr.checks[0].must);
    }

    #[test]
    fn equivalence_policy_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("equiv.yaml");
        std::fs::write(
            &path,
            r#"
version: "1.0.0"
policy_id: EQUIV
requirements:
  - id: EQUIV-001
    rule: fixed_test_ir
    must: true
"#,
        )
        .unwrap();

        let policy = EquivalencePolicy::load(&path).unwrap();
        assert_eq!(policy.requirements.len(), 1);
        assert!(policy.requirements[0].must);
    }

    #[test]
    fn ir_policy_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ir.yaml");
        std::fs::write(
            &path,
            r#"
version: "1.0.0"
policy_id: IR
compatibility_rules:
  - id: IR-001
    must: true
    rule: "Must include ir_schema_version"
required_fields:
  contract_ir: [ir_schema_version]
  test_ir: [ir_schema_version]
"#,
        )
        .unwrap();

        let policy = IrPolicy::load(&path).unwrap();
        assert_eq!(policy.compatibility_rules.len(), 1);
        let rf = policy.required_fields.unwrap();
        assert!(!rf.contract_ir.is_empty());
    }

    #[test]
    fn adversarial_guardrails_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("adv.yaml");
        std::fs::write(
            &path,
            r#"
version: "1.0.0"
policy_id: ADV
requirements:
  - id: ADV-001
    must: true
    rule: "Redact secrets"
"#,
        )
        .unwrap();

        let policy = AdversarialGuardrails::load(&path).unwrap();
        assert_eq!(policy.requirements.len(), 1);
    }

    #[test]
    fn security_constraints_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("security.yaml");
        std::fs::write(
            &path,
            r#"
id: security.global
version: "1.0.0"
owner: team
rules:
  - id: prepared_statements_only
    severity: critical
    statement: "Use prepared statements"
    enforcement: [sast]
exceptions:
  process: "Approval required"
  max_exception_days: 30
"#,
        )
        .unwrap();

        let sc = SecurityConstraints::load(&path).unwrap();
        assert_eq!(sc.rules.len(), 1);
        assert_eq!(sc.rules[0].id, "prepared_statements_only");
        assert!(sc.exceptions.is_some());
        assert_eq!(sc.exceptions.unwrap().max_exception_days, Some(30));
    }

    #[test]
    fn performance_constraints_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("perf.yaml");
        std::fs::write(
            &path,
            r#"
id: perf.global
version: "1.0.0"
defaults:
  latency_p95_ms: 120
  latency_p99_ms: 250
  error_rate_max_percent: 0.5
overrides:
  - contract_id: order.create
    latency_p99_ms: 500
validation:
  warmup_seconds: 30
  fail_on_budget_exceed: true
"#,
        )
        .unwrap();

        let pc = PerformanceConstraints::load(&path).unwrap();
        let d = pc.defaults.unwrap();
        assert_eq!(d.latency_p95_ms, Some(120));
        assert_eq!(pc.overrides.len(), 1);
        assert_eq!(pc.overrides[0].contract_id, "order.create");
    }

    #[test]
    fn gates_policy_add_gate() {
        let mut policy = GatesPolicy {
            version: "1.0.0".to_string(),
            policy_id: "TEST".to_string(),
            description: None,
            release_policy: None,
            gates: vec![],
        };

        let gate = Gate {
            id: "GATE-001".to_string(),
            gate_type: "lint".to_string(),
            mandatory: true,
            enabled: true,
            pass_criteria: None,
            command: Some("cargo clippy".to_string()),
            cwd: None,
            tools: None,
        };
        policy.add_gate(gate).unwrap();
        assert_eq!(policy.gates.len(), 1);

        // Duplicate should fail
        let dup = Gate {
            id: "GATE-001".to_string(),
            gate_type: "lint".to_string(),
            mandatory: false,
            enabled: true,
            pass_criteria: None,
            command: None,
            cwd: None,
            tools: None,
        };
        assert!(policy.add_gate(dup).is_err());
    }

    #[test]
    fn gates_policy_remove_gate() {
        let mut policy = GatesPolicy {
            version: "1.0.0".to_string(),
            policy_id: "TEST".to_string(),
            description: None,
            release_policy: None,
            gates: vec![Gate {
                id: "GATE-001".to_string(),
                gate_type: "lint".to_string(),
                mandatory: true,
                enabled: true,
                pass_criteria: None,
                command: None,
                cwd: None,
                tools: None,
            }],
        };

        let removed = policy.remove_gate("GATE-001").unwrap();
        assert_eq!(removed.id, "GATE-001");
        assert!(policy.gates.is_empty());

        // Not found
        assert!(policy.remove_gate("GATE-999").is_err());
    }

    #[test]
    fn constraint_file_add_remove_rule() {
        let mut cf = ConstraintFile {
            id: "constraints.test.global".to_string(),
            version: "1.0.0".to_string(),
            owner: None,
            intent: None,
            rules: vec![],
            exceptions: None,
        };

        let rule = ConstraintRule {
            id: "rule_1".to_string(),
            severity: "critical".to_string(),
            statement: "Test rule".to_string(),
            enforcement: vec!["sast".to_string()],
        };
        cf.add_rule(rule).unwrap();
        assert_eq!(cf.rules.len(), 1);

        // Duplicate
        let dup = ConstraintRule {
            id: "rule_1".to_string(),
            severity: "high".to_string(),
            statement: "Dup".to_string(),
            enforcement: vec![],
        };
        assert!(cf.add_rule(dup).is_err());

        // Remove
        let removed = cf.remove_rule("rule_1").unwrap();
        assert_eq!(removed.id, "rule_1");
        assert!(cf.rules.is_empty());

        // Not found
        assert!(cf.remove_rule("rule_999").is_err());
    }

    #[test]
    fn constraint_file_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.yaml");

        let cf = ConstraintFile {
            id: "constraints.obs.global".to_string(),
            version: "1.0.0".to_string(),
            owner: Some("platform".to_string()),
            intent: Some("Observability".to_string()),
            rules: vec![ConstraintRule {
                id: "structured_logging".to_string(),
                severity: "critical".to_string(),
                statement: "All logs must be structured".to_string(),
                enforcement: vec!["sast".to_string()],
            }],
            exceptions: Some(ExceptionPolicy {
                process: Some("Approval required".to_string()),
                max_exception_days: Some(30),
            }),
        };

        cf.save(&path).unwrap();
        let loaded = ConstraintFile::load(&path).unwrap();
        assert_eq!(loaded.id, "constraints.obs.global");
        assert_eq!(loaded.rules.len(), 1);
        assert_eq!(loaded.rules[0].id, "structured_logging");
        assert!(loaded.exceptions.is_some());
    }

    #[test]
    fn gates_policy_load_from_fixture() {
        let path =
            std::path::Path::new("tests/fixtures/example-project/validation/gates-policy.yaml");
        let policy = GatesPolicy::load(path).unwrap();
        assert!(!policy.gates.is_empty());
        assert!(policy.gates.iter().any(|g| g.mandatory));
    }
}
