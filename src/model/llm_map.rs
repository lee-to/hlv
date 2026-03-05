use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LlmMap {
    pub schema_version: u32,
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default)]
    pub entries: Vec<MapEntry>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct MapEntry {
    pub path: String,
    pub kind: MapEntryKind,
    pub layer: String,
    pub description: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MapEntryKind {
    File,
    Dir,
}

impl std::fmt::Display for MapEntryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File => write!(f, "file"),
            Self::Dir => write!(f, "dir"),
        }
    }
}

impl LlmMap {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let map: LlmMap = serde_yaml::from_str(&content)?;
        Ok(map)
    }

    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let yaml = serde_yaml::to_string(self)?;
        let content = format!(
            "# yaml-language-server: $schema=../schema/llm-map-schema.json\n# LLM Project Map\n{}",
            yaml
        );
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn add_entry(&mut self, entry: MapEntry) -> anyhow::Result<()> {
        if self.entries.iter().any(|e| e.path == entry.path) {
            anyhow::bail!("Entry '{}' already exists in map", entry.path);
        }
        self.entries.push(entry);
        Ok(())
    }

    pub fn remove_entry(&mut self, path: &str) -> anyhow::Result<MapEntry> {
        let pos = self
            .entries
            .iter()
            .position(|e| e.path == path)
            .ok_or_else(|| anyhow::anyhow!("Entry '{}' not found in map", path))?;
        Ok(self.entries.remove(pos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llm_map_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("map.yaml");

        let map = LlmMap {
            schema_version: 1,
            ignore: vec!["target/**".to_string()],
            entries: vec![MapEntry {
                path: "project.yaml".to_string(),
                kind: MapEntryKind::File,
                layer: "root".to_string(),
                description: "Project map".to_string(),
            }],
        };

        map.save(&path).unwrap();
        let loaded = LlmMap::load(&path).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].path, "project.yaml");
        assert_eq!(loaded.ignore, vec!["target/**"]);
    }

    #[test]
    fn llm_map_add_remove_entry() {
        let mut map = LlmMap {
            schema_version: 1,
            ignore: vec![],
            entries: vec![],
        };

        map.add_entry(MapEntry {
            path: "human/constraints/obs.yaml".to_string(),
            kind: MapEntryKind::File,
            layer: "human".to_string(),
            description: "Observability constraints".to_string(),
        })
        .unwrap();
        assert_eq!(map.entries.len(), 1);

        // Duplicate
        assert!(map
            .add_entry(MapEntry {
                path: "human/constraints/obs.yaml".to_string(),
                kind: MapEntryKind::File,
                layer: "human".to_string(),
                description: "Dup".to_string(),
            })
            .is_err());

        // Remove
        let removed = map.remove_entry("human/constraints/obs.yaml").unwrap();
        assert_eq!(removed.path, "human/constraints/obs.yaml");
        assert!(map.entries.is_empty());

        // Not found
        assert!(map.remove_entry("nope").is_err());
    }
}
