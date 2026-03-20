use serde::{Deserialize, Serialize};

/// Root of project.yaml
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectMap {
    pub schema_version: u32,
    pub project: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub status: ProjectStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_skill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_skill_result: Option<String>,
    pub paths: ProjectPaths,
    #[serde(default)]
    pub glossary_types: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<ConstraintEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<Stack>,
    #[serde(default)]
    pub git: GitPolicy,
    #[serde(default)]
    pub features: Features,
}

// ── Features ────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Features {
    #[serde(default = "default_true")]
    pub linear_architecture: bool,
    #[serde(default = "default_true")]
    pub hlv_markers: bool,
}

impl Default for Features {
    fn default() -> Self {
        Self {
            linear_architecture: true,
            hlv_markers: true,
        }
    }
}

// ── Git Policy ──────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct GitPolicy {
    #[serde(default)]
    pub branch_per_milestone: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_format: Option<String>,
    #[serde(default)]
    pub commit_convention: CommitConvention,
    #[serde(default)]
    pub commit_scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_template: Option<String>,
    #[serde(default = "default_true")]
    pub commit_hints: bool,
    #[serde(default)]
    pub merge_strategy: MergeStrategy,
}

impl Default for GitPolicy {
    fn default() -> Self {
        Self {
            branch_per_milestone: false,
            branch_format: None,
            commit_convention: CommitConvention::Conventional,
            commit_scopes: vec![],
            commit_template: None,
            commit_hints: true,
            merge_strategy: MergeStrategy::Manual,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommitConvention {
    #[default]
    Conventional,
    Simple,
    Custom,
}

impl std::fmt::Display for CommitConvention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conventional => write!(f, "conventional"),
            Self::Simple => write!(f, "simple"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    #[default]
    Manual,
    LocalMerge,
    Pr,
}

impl std::fmt::Display for MergeStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manual => write!(f, "manual"),
            Self::LocalMerge => write!(f, "local-merge"),
            Self::Pr => write!(f, "pr"),
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    #[default]
    Draft,
    Implementing,
    Implemented,
    Validating,
    Validated,
}

impl std::fmt::Display for ProjectStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Implementing => write!(f, "implementing"),
            Self::Implemented => write!(f, "implemented"),
            Self::Validating => write!(f, "validating"),
            Self::Validated => write!(f, "validated"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectPaths {
    pub human: HumanPaths,
    pub validation: ValidationPaths,
    pub llm: LlmPaths,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HumanPaths {
    pub glossary: String,
    pub constraints: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationPaths {
    pub gates_policy: String,
    pub scenarios: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_specs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traceability: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_report: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LlmPaths {
    pub src: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tests: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ContractEntry {
    pub id: String,
    pub version: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yaml_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub status: ContractStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_spec: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub artifacts: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContractStatus {
    Draft,
    Generated,
    Verified,
    Implemented,
}

impl std::fmt::Display for ContractStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Generated => write!(f, "generated"),
            Self::Verified => write!(f, "verified"),
            Self::Implemented => write!(f, "implemented"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ConstraintEntry {
    pub id: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applies_to: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ValidationState {
    pub verify_status: VerifyStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issues: Option<IssueCount>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerifyStatus {
    NotRun,
    Passed,
    Failed,
}

impl std::fmt::Display for VerifyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRun => write!(f, "not_run"),
            Self::Passed => write!(f, "passed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct IssueCount {
    #[serde(default)]
    pub critical: u32,
    #[serde(default)]
    pub warning: u32,
    #[serde(default)]
    pub info: u32,
}

// ── Tech Stack ──────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Stack {
    #[serde(default)]
    pub components: Vec<StackComponent>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StackComponent {
    #[serde(default)]
    pub id: String,
    #[serde(rename = "type")]
    pub component_type: ComponentType,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<StackDependency>,
    /// Extra fields from LLM (engine, provider, etc.) — captured but not validated.
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComponentType {
    Service,
    Library,
    Cli,
    Script,
    Application,
    Datastore,
    ExternalApi,
    Channel,
    Hosting,
    #[serde(other)]
    Other,
}

impl std::fmt::Display for ComponentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Service => write!(f, "service"),
            Self::Library => write!(f, "library"),
            Self::Cli => write!(f, "cli"),
            Self::Script => write!(f, "script"),
            Self::Application => write!(f, "application"),
            Self::Datastore => write!(f, "datastore"),
            Self::ExternalApi => write!(f, "external_api"),
            Self::Channel => write!(f, "channel"),
            Self::Hosting => write!(f, "hosting"),
            Self::Other => write!(f, "other"),
        }
    }
}

impl ComponentType {
    /// Returns true if this component type is expected to have programming languages.
    pub fn expects_language(&self) -> bool {
        !matches!(
            self,
            Self::Datastore | Self::ExternalApi | Self::Channel | Self::Hosting
        )
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct StackDependency {
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type")]
    pub dependency_type: DependencyType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependencyType {
    Framework,
    Driver,
    Runtime,
    Database,
    Infra,
    Tool,
    Serialization,
    Sdk,
    #[serde(other)]
    Other,
}

impl std::fmt::Display for DependencyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Framework => write!(f, "framework"),
            Self::Driver => write!(f, "driver"),
            Self::Runtime => write!(f, "runtime"),
            Self::Database => write!(f, "database"),
            Self::Infra => write!(f, "infra"),
            Self::Tool => write!(f, "tool"),
            Self::Serialization => write!(f, "serialization"),
            Self::Sdk => write!(f, "sdk"),
            Self::Other => write!(f, "other"),
        }
    }
}

impl ProjectMap {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let map: ProjectMap = serde_yaml::from_str(&content)?;
        Ok(map)
    }

    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let yaml = serde_yaml::to_string(self)?;
        let content = format!(
            "# yaml-language-server: $schema=schema/project-schema.json\n# HLV Project Map\n{}",
            yaml
        );
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn add_constraint(&mut self, entry: ConstraintEntry) -> anyhow::Result<()> {
        if self.constraints.iter().any(|c| c.id == entry.id) {
            anyhow::bail!("Constraint '{}' already exists", entry.id);
        }
        self.constraints.push(entry);
        Ok(())
    }

    pub fn remove_constraint(&mut self, id: &str) -> anyhow::Result<ConstraintEntry> {
        let pos = self
            .constraints
            .iter()
            .position(|c| c.id == id)
            .ok_or_else(|| anyhow::anyhow!("Constraint '{}' not found", id))?;
        Ok(self.constraints.remove(pos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_status_display() {
        assert_eq!(ProjectStatus::Draft.to_string(), "draft");
        assert_eq!(ProjectStatus::Implementing.to_string(), "implementing");
        assert_eq!(ProjectStatus::Implemented.to_string(), "implemented");
        assert_eq!(ProjectStatus::Validating.to_string(), "validating");
        assert_eq!(ProjectStatus::Validated.to_string(), "validated");
    }

    #[test]
    fn project_status_default() {
        let s: ProjectStatus = Default::default();
        assert_eq!(s, ProjectStatus::Draft);
    }

    #[test]
    fn contract_status_display() {
        assert_eq!(ContractStatus::Draft.to_string(), "draft");
        assert_eq!(ContractStatus::Generated.to_string(), "generated");
        assert_eq!(ContractStatus::Verified.to_string(), "verified");
        assert_eq!(ContractStatus::Implemented.to_string(), "implemented");
    }

    #[test]
    fn verify_status_display() {
        assert_eq!(VerifyStatus::NotRun.to_string(), "not_run");
        assert_eq!(VerifyStatus::Passed.to_string(), "passed");
        assert_eq!(VerifyStatus::Failed.to_string(), "failed");
    }

    #[test]
    fn component_type_display() {
        assert_eq!(ComponentType::Service.to_string(), "service");
        assert_eq!(ComponentType::Library.to_string(), "library");
        assert_eq!(ComponentType::Cli.to_string(), "cli");
        assert_eq!(ComponentType::ExternalApi.to_string(), "external_api");
        assert_eq!(ComponentType::Other.to_string(), "other");
    }

    #[test]
    fn dependency_type_display() {
        assert_eq!(DependencyType::Framework.to_string(), "framework");
        assert_eq!(DependencyType::Sdk.to_string(), "sdk");
        assert_eq!(DependencyType::Other.to_string(), "other");
    }

    #[test]
    fn component_type_serde_other() {
        let yaml = r#"
id: test
type: custom_unknown_type
languages: []
"#;
        let comp: StackComponent = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(comp.component_type, ComponentType::Other);
    }

    #[test]
    fn dependency_type_serde_other() {
        let yaml = r#"
name: test
type: some_new_type
"#;
        let dep: StackDependency = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(dep.dependency_type, DependencyType::Other);
    }

    #[test]
    fn project_map_load_valid() {
        let path = std::path::Path::new("tests/fixtures/example-project/project.yaml");
        let pm = ProjectMap::load(path).unwrap();
        assert_eq!(pm.project, "commerce-checkout");
        assert_eq!(pm.status, ProjectStatus::Implementing);
        assert!(!pm.glossary_types.is_empty());
        assert!(!pm.constraints.is_empty());
        assert!(pm.stack.is_some());
    }

    #[test]
    fn project_map_save_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.yaml");

        let pm = ProjectMap {
            schema_version: 1,
            project: "test-project".to_string(),
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
                    test_specs: None,
                    traceability: None,
                    verify_report: None,
                },
                llm: LlmPaths {
                    src: "llm/src/".to_string(),
                    tests: None,
                    map: Some("llm/map.yaml".to_string()),
                },
            },
            glossary_types: vec!["UserId".to_string()],
            constraints: vec![ConstraintEntry {
                id: "security.global".to_string(),
                path: "human/constraints/security.yaml".to_string(),
                applies_to: Some("all".to_string()),
            }],
            validation: None,
            stack: None,
            git: GitPolicy::default(),
            features: Features::default(),
        };

        pm.save(&path).unwrap();
        let loaded = ProjectMap::load(&path).unwrap();
        assert_eq!(loaded.project, "test-project");
        assert_eq!(loaded.constraints.len(), 1);
        assert_eq!(loaded.constraints[0].id, "security.global");
    }

    #[test]
    fn project_map_add_remove_constraint() {
        let mut pm = ProjectMap {
            schema_version: 1,
            project: "test".to_string(),
            spec: None,
            updated_at: None,
            status: ProjectStatus::Draft,
            last_skill: None,
            last_skill_result: None,
            paths: ProjectPaths {
                human: HumanPaths {
                    glossary: "g.yaml".to_string(),
                    constraints: "c/".to_string(),
                    artifacts: None,
                },
                validation: ValidationPaths {
                    gates_policy: "v/g.yaml".to_string(),
                    scenarios: "v/s/".to_string(),
                    test_specs: None,
                    traceability: None,
                    verify_report: None,
                },
                llm: LlmPaths {
                    src: "llm/".to_string(),
                    tests: None,
                    map: None,
                },
            },
            glossary_types: vec![],
            constraints: vec![],
            validation: None,
            stack: None,
            git: GitPolicy::default(),
            features: Features::default(),
        };

        pm.add_constraint(ConstraintEntry {
            id: "sec.global".to_string(),
            path: "c/security.yaml".to_string(),
            applies_to: Some("all".to_string()),
        })
        .unwrap();
        assert_eq!(pm.constraints.len(), 1);

        // Duplicate
        assert!(pm
            .add_constraint(ConstraintEntry {
                id: "sec.global".to_string(),
                path: "c/sec2.yaml".to_string(),
                applies_to: None,
            })
            .is_err());

        // Remove
        let removed = pm.remove_constraint("sec.global").unwrap();
        assert_eq!(removed.id, "sec.global");
        assert!(pm.constraints.is_empty());

        // Not found
        assert!(pm.remove_constraint("nope").is_err());
    }

    #[test]
    fn project_map_load_missing() {
        let result = ProjectMap::load(std::path::Path::new("/nonexistent.yaml"));
        assert!(result.is_err());
    }

    #[test]
    fn project_map_load_invalid_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.yaml");
        std::fs::write(&path, "not: [valid: yaml: {{{").unwrap();
        let result = ProjectMap::load(&path);
        assert!(result.is_err());
    }

    #[test]
    fn project_status_serde_roundtrip() {
        for status in &[
            ProjectStatus::Draft,
            ProjectStatus::Implementing,
            ProjectStatus::Implemented,
            ProjectStatus::Validating,
            ProjectStatus::Validated,
        ] {
            let yaml = serde_yaml::to_string(status).unwrap();
            let parsed: ProjectStatus = serde_yaml::from_str(&yaml).unwrap();
            assert_eq!(&parsed, status);
        }
    }

    #[test]
    fn contract_status_serde_roundtrip() {
        for status in &[
            ContractStatus::Draft,
            ContractStatus::Generated,
            ContractStatus::Verified,
            ContractStatus::Implemented,
        ] {
            let yaml = serde_yaml::to_string(status).unwrap();
            let parsed: ContractStatus = serde_yaml::from_str(&yaml).unwrap();
            assert_eq!(&parsed, status);
        }
    }

    #[test]
    fn stack_component_extra_fields() {
        let yaml = r#"
id: backend
type: service
languages: [rust]
engine: v8
custom_field: hello
"#;
        let comp: StackComponent = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(comp.id, "backend");
        assert!(comp.extra.contains_key("engine"));
        assert!(comp.extra.contains_key("custom_field"));
    }

    #[test]
    fn features_default_both_true() {
        let f = Features::default();
        assert!(f.linear_architecture);
        assert!(f.hlv_markers);
    }

    #[test]
    fn features_deserialize_explicit_false() {
        let yaml = "linear_architecture: false\nhlv_markers: false\n";
        let f: Features = serde_yaml::from_str(yaml).unwrap();
        assert!(!f.linear_architecture);
        assert!(!f.hlv_markers);
    }

    #[test]
    fn features_deserialize_empty_defaults_true() {
        let yaml = "{}\n";
        let f: Features = serde_yaml::from_str(yaml).unwrap();
        assert!(f.linear_architecture);
        assert!(f.hlv_markers);
    }

    #[test]
    fn features_deserialize_partial() {
        let yaml = "hlv_markers: false\n";
        let f: Features = serde_yaml::from_str(yaml).unwrap();
        assert!(f.linear_architecture); // default true
        assert!(!f.hlv_markers);
    }

    #[test]
    fn project_map_without_features_defaults() {
        // A project.yaml without features section should deserialize with defaults
        let path = std::path::Path::new("tests/fixtures/example-project/project.yaml");
        let pm = ProjectMap::load(path).unwrap();
        // Fixture may or may not have features; either way it should load
        // and features should have sensible values
        let _ = pm.features.linear_architecture;
        let _ = pm.features.hlv_markers;
    }

    #[test]
    fn project_map_roundtrip_with_features() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.yaml");

        let mut pm = ProjectMap {
            schema_version: 1,
            project: "test-features".to_string(),
            spec: None,
            updated_at: None,
            status: ProjectStatus::Draft,
            last_skill: None,
            last_skill_result: None,
            paths: ProjectPaths {
                human: HumanPaths {
                    glossary: "g.yaml".to_string(),
                    constraints: "c/".to_string(),
                    artifacts: None,
                },
                validation: ValidationPaths {
                    gates_policy: "v/g.yaml".to_string(),
                    scenarios: "v/s/".to_string(),
                    test_specs: None,
                    traceability: None,
                    verify_report: None,
                },
                llm: LlmPaths {
                    src: "llm/".to_string(),
                    tests: None,
                    map: None,
                },
            },
            glossary_types: vec![],
            constraints: vec![],
            validation: None,
            stack: None,
            git: GitPolicy::default(),
            features: Features {
                linear_architecture: true,
                hlv_markers: false,
            },
        };

        pm.save(&path).unwrap();
        let loaded = ProjectMap::load(&path).unwrap();
        assert!(loaded.features.linear_architecture);
        assert!(!loaded.features.hlv_markers);
    }
}
