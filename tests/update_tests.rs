use std::fs;
use std::process::Command;

use tempfile::TempDir;

fn init_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    hlv::cmd::init::run_with_milestone(
        tmp.path().to_str().unwrap(),
        Some("update-test"),
        Some("team"),
        Some("claude"),
        Some("init"),
        Some("minimal"),
    )
    .unwrap();
    tmp
}

#[test]
fn project_only_update_refreshes_managed_files_without_changing_project_data() {
    let tmp = init_project();
    let project_yaml = tmp.path().join("project.yaml");
    let skill = tmp.path().join(".claude/skills/hlv-generate/SKILL.md");
    let original_project = fs::read(&project_yaml).unwrap();

    fs::write(&skill, [0xff, 0xfe]).unwrap();
    fs::write(tmp.path().join("HLV.md"), "outdated").unwrap();

    hlv::cmd::update::run(false, false, true, tmp.path().to_str()).unwrap();

    let refreshed_skill = fs::read_to_string(skill).unwrap();
    assert!(refreshed_skill.contains("hlv-generate"));
    assert_ne!(
        fs::read_to_string(tmp.path().join("HLV.md")).unwrap(),
        "outdated"
    );
    assert_eq!(fs::read(project_yaml).unwrap(), original_project);
}

#[test]
fn project_only_check_does_not_modify_managed_files() {
    let tmp = init_project();
    let skill = tmp.path().join(".claude/skills/hlv-generate/SKILL.md");
    fs::write(&skill, "locally changed").unwrap();

    hlv::cmd::update::run(true, false, true, tmp.path().to_str()).unwrap();

    assert_eq!(fs::read_to_string(skill).unwrap(), "locally changed");
}

#[test]
fn project_only_update_requires_an_hlv_project() {
    let tmp = TempDir::new().unwrap();

    let error = hlv::cmd::update::run(false, false, true, tmp.path().to_str()).unwrap_err();

    assert!(error.to_string().contains("No project.yaml"));
}

#[test]
fn binary_only_and_project_only_flags_conflict() {
    let output = Command::new(env!("CARGO_BIN_EXE_hlv"))
        .args(["update", "--binary-only", "--project-only"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used with"));
}
