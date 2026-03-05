use serde::{Deserialize, Serialize};

/// Contract YAML IR — human/milestones/{id}/contracts/*.yaml
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractYaml {
    pub id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs_schema: Option<serde_yaml::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outputs_schema: Option<serde_yaml::Value>,
    #[serde(default)]
    pub errors: Vec<ContractError>,
    #[serde(default)]
    pub invariants: Vec<ContractInvariant>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nfr: Option<ContractNfr>,
    #[serde(default)]
    pub security: Vec<SecurityRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<Compatibility>,
    #[serde(default)]
    pub depends_on_constraints: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ContractError {
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ContractInvariant {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ContractNfr {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_p99_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub availability_slo: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub throughput_rps_min: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_query_time_ms: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SecurityRule {
    pub rule: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Compatibility {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_semver: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backward_compatible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_migration_required: Option<bool>,
}

impl ContractYaml {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let contract: ContractYaml = serde_yaml::from_str(&content)?;
        Ok(contract)
    }
}
