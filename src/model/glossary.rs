use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Root of human/glossary.yaml
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Glossary {
    #[serde(default)]
    pub schema_version: Option<u32>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub types: HashMap<String, GlossaryType>,
    #[serde(default)]
    pub enums: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub terms: HashMap<String, GlossaryTerm>,
    #[serde(default)]
    pub rules: Vec<GlossaryRule>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct GlossaryType {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fields: Option<HashMap<String, serde_yaml::Value>>,
    #[serde(rename = "enum", default, skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct GlossaryTerm {
    pub canonical: String,
    pub definition: String,
    #[serde(default)]
    pub aliases_forbidden: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct GlossaryRule {
    pub id: String,
    pub description: String,
}

impl Glossary {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let glossary: Glossary = serde_yaml::from_str(&content)?;
        Ok(glossary)
    }

    /// Get all known type names (types + enums)
    pub fn all_type_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.types.keys().map(|s| s.as_str()).collect();
        names.extend(self.enums.keys().map(|s| s.as_str()));
        names
    }
}
