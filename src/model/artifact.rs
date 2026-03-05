use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Kind of artifact based on filename
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Context,
    Stack,
    Constraints,
    Decision,
    Feature,
    Unknown,
}

/// Lightweight artifact metadata (no content loaded)
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactMeta {
    pub name: String,
    pub path: PathBuf,
    pub kind: ArtifactKind,
}

/// Full artifact with content
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactFull {
    pub name: String,
    pub path: PathBuf,
    pub kind: ArtifactKind,
    pub content: String,
}

/// Index of all artifacts (global + milestone)
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactIndex {
    pub global: Vec<ArtifactMeta>,
    pub milestone: Vec<ArtifactMeta>,
}

impl ArtifactIndex {
    /// Load metadata for all artifacts
    pub fn load(root: &Path, milestone_id: Option<&str>) -> Result<Self> {
        let global = Self::load_global(root)?;
        let milestone = match milestone_id {
            Some(mid) => Self::load_milestone(root, mid)?,
            None => Vec::new(),
        };
        Ok(Self { global, milestone })
    }

    /// Load global artifacts from human/artifacts/
    pub fn load_global(root: &Path) -> Result<Vec<ArtifactMeta>> {
        let dir = root.join("human/artifacts");
        scan_artifacts_dir(&dir)
    }

    /// Load milestone artifacts from human/milestones/{mid}/artifacts/
    pub fn load_milestone(root: &Path, mid: &str) -> Result<Vec<ArtifactMeta>> {
        let dir = root.join("human/milestones").join(mid).join("artifacts");
        scan_artifacts_dir(&dir)
    }
}

impl ArtifactFull {
    /// Load a single artifact with content
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let kind = classify_artifact(&name);
        Ok(Self {
            name,
            path: path.to_path_buf(),
            kind,
            content,
        })
    }
}

fn scan_artifacts_dir(dir: &Path) -> Result<Vec<ArtifactMeta>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().map(|e| e == "md").unwrap_or(false) {
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let kind = classify_artifact(&name);
            result.push(ArtifactMeta { name, path, kind });
        }
    }
    result.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(result)
}

fn classify_artifact(name: &str) -> ArtifactKind {
    match name {
        "context" => ArtifactKind::Context,
        "stack" => ArtifactKind::Stack,
        "constraints" => ArtifactKind::Constraints,
        n if n.starts_with("feature") || n.starts_with("feat") => ArtifactKind::Feature,
        n if n.starts_with("decision") || n.starts_with("adr") => ArtifactKind::Decision,
        _ => ArtifactKind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn classify_known_artifacts() {
        assert_eq!(classify_artifact("context"), ArtifactKind::Context);
        assert_eq!(classify_artifact("stack"), ArtifactKind::Stack);
        assert_eq!(classify_artifact("constraints"), ArtifactKind::Constraints);
        assert_eq!(classify_artifact("feature-auth"), ArtifactKind::Feature);
        assert_eq!(classify_artifact("decision-api"), ArtifactKind::Decision);
        assert_eq!(classify_artifact("something"), ArtifactKind::Unknown);
    }

    #[test]
    fn load_global_artifacts() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let dir = root.join("human/artifacts");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("context.md"), "# Context").unwrap();
        std::fs::write(dir.join("stack.md"), "# Stack").unwrap();
        std::fs::write(dir.join("notes.txt"), "not an artifact").unwrap();

        let artifacts = ArtifactIndex::load_global(root).unwrap();
        assert_eq!(artifacts.len(), 2);
        assert_eq!(artifacts[0].name, "context");
        assert_eq!(artifacts[0].kind, ArtifactKind::Context);
        assert_eq!(artifacts[1].name, "stack");
    }

    #[test]
    fn load_full_artifact() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("context.md");
        std::fs::write(&path, "# Context\n\nSome content").unwrap();

        let full = ArtifactFull::load(&path).unwrap();
        assert_eq!(full.name, "context");
        assert_eq!(full.kind, ArtifactKind::Context);
        assert!(full.content.contains("Some content"));
    }

    #[test]
    fn empty_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let artifacts = ArtifactIndex::load_global(tmp.path()).unwrap();
        assert!(artifacts.is_empty());
    }
}
