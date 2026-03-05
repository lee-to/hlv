use hlv::model::milestone::{MilestoneGitConfig, MilestoneMap};
use hlv::model::project::{CommitConvention, GitPolicy, MergeStrategy, ProjectMap};

#[test]
fn git_policy_defaults_when_missing() {
    // project.yaml without git section uses defaults
    let yaml = r#"
schema_version: 1
project: test
status: draft
paths:
  human:
    glossary: human/glossary.yaml
    constraints: human/constraints/
    artifacts: human/artifacts/
  validation:
    gates_policy: validation/gates-policy.yaml
    scenarios: validation/scenarios/
    test_specs: validation/test-specs/
    traceability: human/traceability.yaml
  llm:
    src: llm/src/
"#;
    let pm: ProjectMap = serde_yaml::from_str(yaml).unwrap();
    assert!(!pm.git.branch_per_milestone);
    assert_eq!(pm.git.commit_convention, CommitConvention::Conventional);
    assert_eq!(pm.git.merge_strategy, MergeStrategy::Manual);
}

#[test]
fn git_policy_parses_full() {
    let yaml = r#"
schema_version: 1
project: test
status: draft
paths:
  human:
    glossary: g.yaml
    constraints: c/
    artifacts: a/
  validation:
    gates_policy: v/g.yaml
    scenarios: v/s/
    test_specs: v/t/
    traceability: t.yaml
  llm:
    src: llm/
git:
  branch_per_milestone: true
  branch_format: "feature/{milestone-slug}"
  commit_convention: conventional
  commit_scopes:
    - feat
    - fix
    - refactor
  commit_hints: true
  merge_strategy: manual
"#;
    let pm: ProjectMap = serde_yaml::from_str(yaml).unwrap();
    assert!(pm.git.branch_per_milestone);
    assert_eq!(
        pm.git.branch_format.as_deref(),
        Some("feature/{milestone-slug}")
    );
    assert_eq!(pm.git.commit_convention, CommitConvention::Conventional);
    assert_eq!(pm.git.commit_scopes.len(), 3);
    assert!(pm.git.commit_hints);
    assert_eq!(pm.git.merge_strategy, MergeStrategy::Manual);
}

#[test]
fn git_policy_serde_roundtrip() {
    let gp = GitPolicy {
        branch_per_milestone: true,
        branch_format: Some("hlv/{milestone-id}".to_string()),
        commit_convention: CommitConvention::Simple,
        commit_scopes: vec!["feat".to_string(), "fix".to_string()],
        commit_template: None,
        commit_hints: true,
        merge_strategy: MergeStrategy::Pr,
    };

    let yaml = serde_yaml::to_string(&gp).unwrap();
    let parsed: GitPolicy = serde_yaml::from_str(&yaml).unwrap();
    assert!(parsed.branch_per_milestone);
    assert_eq!(parsed.commit_convention, CommitConvention::Simple);
    assert_eq!(parsed.merge_strategy, MergeStrategy::Pr);
}

#[test]
fn git_policy_defaults() {
    let gp = GitPolicy::default();
    assert!(!gp.branch_per_milestone);
    assert!(gp.commit_hints);
    assert_eq!(gp.commit_convention, CommitConvention::Conventional);
    assert_eq!(gp.merge_strategy, MergeStrategy::Manual);
    assert!(gp.commit_scopes.is_empty());
}

#[test]
fn milestone_git_config_optional() {
    let yaml = r#"
project: test
current:
  id: "001-test"
  number: 1
  stages: []
"#;
    let m: MilestoneMap = serde_yaml::from_str(yaml).unwrap();
    let current = m.current.unwrap();
    assert!(current.git.is_none());
}

#[test]
fn milestone_git_config_parses() {
    let yaml = r#"
project: test
current:
  id: "001-test"
  number: 1
  stages: []
  git:
    branch_per_milestone: true
    squash_on_merge: false
"#;
    let m: MilestoneMap = serde_yaml::from_str(yaml).unwrap();
    let git = m.current.unwrap().git.unwrap();
    assert_eq!(git.branch_per_milestone, Some(true));
    assert_eq!(git.squash_on_merge, Some(false));
}

#[test]
fn milestone_git_config_roundtrip() {
    let cfg = MilestoneGitConfig {
        branch_per_milestone: Some(true),
        squash_on_merge: Some(true),
    };
    let yaml = serde_yaml::to_string(&cfg).unwrap();
    let parsed: MilestoneGitConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(parsed.branch_per_milestone, Some(true));
    assert_eq!(parsed.squash_on_merge, Some(true));
}

#[test]
fn commit_convention_display() {
    assert_eq!(CommitConvention::Conventional.to_string(), "conventional");
    assert_eq!(CommitConvention::Simple.to_string(), "simple");
    assert_eq!(CommitConvention::Custom.to_string(), "custom");
}

#[test]
fn merge_strategy_display() {
    assert_eq!(MergeStrategy::Manual.to_string(), "manual");
    assert_eq!(MergeStrategy::LocalMerge.to_string(), "local-merge");
    assert_eq!(MergeStrategy::Pr.to_string(), "pr");
}

#[test]
fn commit_msg_conventional() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    hlv::cmd::init::run_with_milestone(
        root.to_str().unwrap(),
        Some("test"),
        Some("qa"),
        Some("claude"),
        Some("init"),
        Some("minimal"),
    )
    .unwrap();

    // Just verify it doesn't panic
    hlv::cmd::commit_msg::run(root, false, None).unwrap();
    hlv::cmd::commit_msg::run(root, true, None).unwrap();
    hlv::cmd::commit_msg::run(root, false, Some("fix")).unwrap();
}
