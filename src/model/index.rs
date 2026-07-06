use serde::{Deserialize, Serialize};

/// Signature index stored at index/signatures.yaml or .hlv/index/signatures.yaml.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Index {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default)]
    pub symbols: Vec<Symbol>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Symbol {
    pub id: String,
    pub name: String,
    pub file: String,
    pub line: u32,
    pub signature: String,
    pub visibility: String,
    pub kind: String,
    pub language: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    pub source_fingerprint: String,
}

impl Index {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_yaml::from_str(&content)?)
    }

    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let yaml = serde_yaml::to_string(self)?;
        let content = format!(
            "# yaml-language-server: $schema=../schema/signatures-schema.json\n{}",
            yaml
        );
        std::fs::write(path, content)?;
        Ok(())
    }
}
