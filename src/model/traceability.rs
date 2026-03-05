use serde::{Deserialize, Serialize};

/// human/traceability.yaml
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TraceabilityMap {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    #[serde(default)]
    pub requirements: Vec<Requirement>,
    #[serde(default)]
    pub mappings: Vec<TraceMapping>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage_policy: Option<CoveragePolicy>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub id: String,
    pub statement: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct TraceMapping {
    pub requirement: String,
    #[serde(default)]
    pub contracts: Vec<String>,
    #[serde(default)]
    pub scenarios: Vec<String>,
    #[serde(default)]
    pub tests: Vec<String>,
    #[serde(default)]
    pub runtime_gates: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CoveragePolicy {
    #[serde(default)]
    pub require_full_traceability: bool,
    #[serde(default)]
    pub allow_unmapped_requirements: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_mandatory_gate_coverage_percent: Option<u32>,
}

impl TraceabilityMap {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_yaml::from_str(&content)?)
    }
}
