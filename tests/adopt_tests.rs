//! Integration tests for adopted projects (HLV config under `.hlv/`).
//!
//! These use a minimal hand-written `.hlv/` layout with only currently
//! supported project.yaml fields. Full adopted fixtures with
//! `features.legacy_mode` / `paths.code` live in `tests/fixtures/adopt-*`.

use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

use hlv::cmd::init::AdoptManifestKind;

use hlv::cmd::check::{get_check_report, CheckOptions};
use hlv::cmd::doctor::doctor_report;
use hlv::cmd::status::get_status;

fn hlv_binary() -> &'static str {
    env!("CARGO_BIN_EXE_hlv")
}

/// Write a minimal adopted project: HLV artifacts under `root/.hlv/`,
/// observed code at the repository root.
fn write_minimal_adopted_project(root: &Path) {
    // Observed brownfield code at the repo root
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(root.join("app/main.py"), "def main():\n    pass\n").unwrap();
    fs::write(root.join("pyproject.toml"), "[project]\nname = \"demo\"\n").unwrap();

    // HLV config artifacts under .hlv/
    let hlv = root.join(".hlv");
    fs::create_dir_all(hlv.join("human/constraints")).unwrap();
    fs::create_dir_all(hlv.join("validation/scenarios")).unwrap();
    fs::create_dir_all(hlv.join("llm/src")).unwrap();
    fs::create_dir_all(hlv.join("llm/tests")).unwrap();
    fs::write(
        hlv.join("human/glossary.yaml"),
        "schema_version: 1\ntypes: {}\nenums: {}\n",
    )
    .unwrap();
    fs::write(
        hlv.join("validation/gates-policy.yaml"),
        "version: 1.0.0\npolicy_id: ADOPT-TEST\ngates: []\n",
    )
    .unwrap();
    fs::write(hlv.join("llm/map.yaml"), "schema_version: 1\nentries: []\n").unwrap();
    fs::write(
        hlv.join("milestones.yaml"),
        "project: adopted-demo\ncurrent:\nhistory: []\n",
    )
    .unwrap();
    fs::write(
        hlv.join("project.yaml"),
        r#"schema_version: 1
project: adopted-demo
status: draft
paths:
  human:
    glossary: human/glossary.yaml
    constraints: human/constraints/
  validation:
    gates_policy: validation/gates-policy.yaml
    scenarios: validation/scenarios/
  llm:
    src: llm/src/
    tests: llm/tests/
    map: llm/map.yaml
git:
  commit_convention: conventional
  merge_strategy: manual
"#,
    )
    .unwrap();
}

#[test]
fn find_project_root_discovers_adopted_layout_from_nested_dir() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    write_minimal_adopted_project(&root);

    let found = hlv::find_project_root_from(&root.join("app")).unwrap();
    assert_eq!(found.canonicalize().unwrap(), root);
    assert_eq!(hlv::config_root(&found), found.join(".hlv"));
}

#[test]
fn find_project_root_normalizes_from_hlv_owned_dirs() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    write_minimal_adopted_project(&root);

    for relative in ["human", "validation"] {
        let found = hlv::find_project_root_from(&root.join(".hlv").join(relative)).unwrap();
        assert_eq!(found.canonicalize().unwrap(), root, "{relative}");
        assert_eq!(hlv::config_root(&found), found.join(".hlv"));
    }
}

#[test]
fn find_project_root_normalizes_explicit_hlv_root() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    write_minimal_adopted_project(&root);

    let found = hlv::find_project_root(Some(root.join(".hlv").to_str().unwrap())).unwrap();
    assert_eq!(found.canonicalize().unwrap(), root);
    assert_eq!(
        hlv::ProjectContext::from_root(&root.join(".hlv")).repo_root(),
        root
    );
}

#[test]
fn check_runs_against_adopted_layout() {
    let tmp = TempDir::new().unwrap();
    write_minimal_adopted_project(tmp.path());

    let report = get_check_report(tmp.path(), CheckOptions::default()).unwrap();
    // No fatal "project.yaml not found" — config was read from .hlv/
    assert!(
        !report.diagnostics.iter().any(|d| d.code == "PRJ-001"),
        "PRJ-001 should not fire for adopted layout: {:?}",
        report.diagnostics
    );
}

#[test]
fn check_watch_paths_resolve_under_hlv_for_adopted_layout() {
    let tmp = TempDir::new().unwrap();
    write_minimal_adopted_project(tmp.path());

    let paths = hlv::cmd::check::watch_paths_for_project(tmp.path());
    assert_eq!(
        paths,
        vec![
            tmp.path().join(".hlv/human"),
            tmp.path().join(".hlv/validation"),
            tmp.path().join(".hlv/project.yaml"),
        ]
    );
}

#[test]
fn status_runs_against_adopted_layout() {
    let tmp = TempDir::new().unwrap();
    write_minimal_adopted_project(tmp.path());

    let status = get_status(tmp.path()).unwrap();
    assert_eq!(status.project, "adopted-demo");
}

#[test]
fn doctor_runs_against_adopted_layout() {
    let tmp = TempDir::new().unwrap();
    write_minimal_adopted_project(tmp.path());

    let report = doctor_report(tmp.path(), false).unwrap();
    assert!(
        !report.diagnostics.iter().any(|d| d.code == "DOC-001"),
        "DOC-001 should not fire for adopted layout: {:?}",
        report.diagnostics
    );
    // Directory-existence checks must resolve under .hlv/, not the repo root
    assert!(
        !report.diagnostics.iter().any(|d| d.code == "DOC-021"),
        "DOC-021 should not fire when dirs exist under .hlv/: {:?}",
        report.diagnostics
    );
}

#[test]
fn workspace_add_accepts_adopted_layout() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path().join("workspace.yaml");
    let proj = tmp.path().join("adopted-proj");
    fs::create_dir(&proj).unwrap();
    write_minimal_adopted_project(&proj);

    hlv::cmd::workspace::run_init(Some(ws.to_str().unwrap())).unwrap();
    hlv::cmd::workspace::run_add(
        Some("adopted-proj"),
        Some(proj.to_str().unwrap()),
        Some(ws.to_str().unwrap()),
    )
    .unwrap();

    let config = hlv::mcp::workspace::WorkspaceConfig::load_lenient(&ws).unwrap();
    assert_eq!(config.projects.len(), 1);
    assert_eq!(config.projects[0].id, "adopted-proj");
}

#[test]
fn mcp_resource_read_works_for_adopted_layout() {
    let tmp = TempDir::new().unwrap();
    write_minimal_adopted_project(tmp.path());

    let result = hlv::mcp::resources::read_resource(tmp.path(), "hlv://project").unwrap();
    let text = match &result.contents[0] {
        rmcp::model::ResourceContents::TextResourceContents { text, .. } => text.clone(),
        other => panic!("expected text contents, got {other:?}"),
    };
    assert!(
        text.contains("adopted-demo"),
        "project resource should load from .hlv/: {text}"
    );
}

#[test]
fn adopt_init_writes_hlv_owned_files_under_hlv_dir() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();

    hlv::cmd::init::run_with_options(
        tmp.path().to_str().unwrap(),
        Some("adopted-init"),
        Some("team"),
        Some("claude"),
        Some("adopt-test"),
        Some("minimal"),
        true,
    )
    .unwrap();

    // HLV-owned files under .hlv/
    assert!(tmp.path().join(".hlv/project.yaml").exists());
    assert!(tmp.path().join(".hlv/milestones.yaml").exists());
    assert!(tmp.path().join(".hlv/human/glossary.yaml").exists());
    assert!(tmp
        .path()
        .join(".hlv/validation/gates-policy.yaml")
        .exists());
    assert!(tmp.path().join(".hlv/llm/map.yaml").exists());
    assert!(tmp.path().join(".hlv/index").is_dir());
    assert!(tmp.path().join(".hlv/schema/project-schema.json").exists());
    assert!(tmp
        .path()
        .join(".hlv/schema/signatures-schema.json")
        .exists());
    // Root-owned files stay at the repository root
    assert!(tmp.path().join("AGENTS.md").exists());
    assert!(tmp.path().join("HLV.md").exists());
    assert!(tmp.path().join(".claude/skills").is_dir());
    let agents_md = fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap();
    assert!(agents_md.contains("IDX-010"));
    assert!(agents_md.contains("Adopt mode checklist"));
    assert!(fs::read_to_string(tmp.path().join(".gitignore"))
        .unwrap()
        .contains(".hlv/index/"));
    // No greenfield project.yaml at the root
    assert!(!tmp.path().join("project.yaml").exists());

    let project =
        hlv::model::project::ProjectMap::load(&tmp.path().join(".hlv/project.yaml")).unwrap();
    assert_eq!(project.hlv_root.as_deref(), Some(".hlv"));
    assert!(project.features.legacy_mode);
    assert!(!project.features.hlv_markers);
    assert!(!project.features.security_markers);
    assert_eq!(
        project.features.index_tracking,
        hlv::model::project::IndexTrackingPolicy::Ignored
    );
    assert!(project.paths.code.is_some());
}

#[test]
fn init_detects_existing_project_manifests() {
    let cases = [
        ("composer.json", AdoptManifestKind::Composer),
        ("go.mod", AdoptManifestKind::Go),
        ("package.json", AdoptManifestKind::Node),
        ("pyproject.toml", AdoptManifestKind::Python),
        ("Cargo.toml", AdoptManifestKind::Rust),
    ];

    for (manifest, expected) in cases {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(manifest), "").unwrap();

        let detected = hlv::cmd::init::detect_adopt_manifests(tmp.path());

        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].kind, expected);
        assert_eq!(detected[0].path, manifest);
    }
}

#[test]
fn init_defaults_to_adopt_when_manifest_is_present() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();

    hlv::cmd::init::run_auto(
        tmp.path().to_str().unwrap(),
        Some("auto-adopt"),
        Some("team"),
        Some("claude"),
        Some("minimal"),
        false,
        false,
    )
    .unwrap();

    assert!(tmp.path().join(".hlv/project.yaml").exists());
    assert!(!tmp.path().join("project.yaml").exists());
}

#[test]
fn init_greenfield_opt_out_overrides_manifest_default() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();

    hlv::cmd::init::run_auto(
        tmp.path().to_str().unwrap(),
        Some("greenfield"),
        Some("team"),
        Some("claude"),
        Some("minimal"),
        false,
        true,
    )
    .unwrap();

    assert!(tmp.path().join("project.yaml").exists());
    assert!(!tmp.path().join(".hlv/project.yaml").exists());
}

#[test]
fn init_explicit_adopt_rejects_existing_root_project() {
    let tmp = TempDir::new().unwrap();
    hlv::cmd::init::run_with_options(
        tmp.path().to_str().unwrap(),
        Some("root-project"),
        Some("team"),
        Some("claude"),
        Some("init"),
        Some("minimal"),
        false,
    )
    .unwrap();

    let err = hlv::cmd::init::run_auto(
        tmp.path().to_str().unwrap(),
        Some("root-project"),
        Some("team"),
        Some("claude"),
        Some("minimal"),
        true,
        false,
    )
    .unwrap_err();

    assert!(err.to_string().contains("Cannot use --adopt"));
    assert!(!tmp.path().join(".hlv/project.yaml").exists());
}

#[test]
fn init_adopt_generates_stack_specific_defaults() {
    struct Case {
        name: &'static str,
        files: &'static [(&'static str, &'static str)],
        dirs: &'static [&'static str],
        source_roots: &'static [&'static str],
        test_roots: &'static [&'static str],
        command: &'static str,
        languages: &'static [&'static str],
    }

    let cases = [
        Case {
            name: "laravel",
            files: &[("composer.json", "{}"), ("artisan", "")],
            dirs: &["app", "routes", "tests"],
            source_roots: &["app/", "routes/"],
            test_roots: &["tests/"],
            command: "php artisan test",
            languages: &["php"],
        },
        Case {
            name: "go",
            files: &[("go.mod", "module example.com/app\n")],
            dirs: &["cmd", "internal", "pkg", "tests"],
            source_roots: &["cmd/", "internal/", "pkg/"],
            test_roots: &["tests/"],
            command: "go test ./...",
            languages: &["go"],
        },
        Case {
            name: "node",
            files: &[("package.json", r#"{"scripts":{"test":"vitest run"}}"#)],
            dirs: &["src", "test"],
            source_roots: &["src/"],
            test_roots: &["test/"],
            command: "npm test",
            languages: &["javascript"],
        },
        Case {
            name: "python",
            files: &[
                ("pyproject.toml", "[project]\nname = \"app\"\n"),
                ("service/__init__.py", ""),
            ],
            dirs: &["service", "tests"],
            source_roots: &["service/"],
            test_roots: &["tests/"],
            command: "pytest",
            languages: &["python"],
        },
        Case {
            name: "rust",
            files: &[("Cargo.toml", "[package]\nname = \"app\"\n")],
            dirs: &["src", "tests"],
            source_roots: &["src/"],
            test_roots: &["tests/"],
            command: "cargo test",
            languages: &["rust"],
        },
        Case {
            name: "mixed-go-node",
            files: &[
                ("go.mod", "module example.com/app\n"),
                ("package.json", r#"{"scripts":{"test":"npm run check"}}"#),
            ],
            dirs: &["cmd", "internal", "src", "test"],
            source_roots: &["cmd/", "internal/", "src/"],
            test_roots: &["test/"],
            command: "go test ./...",
            languages: &["go", "javascript"],
        },
    ];

    for case in cases {
        let tmp = TempDir::new().unwrap();
        for dir in case.dirs {
            fs::create_dir_all(tmp.path().join(dir)).unwrap();
        }
        for (path, content) in case.files {
            let path = tmp.path().join(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, content).unwrap();
        }

        hlv::cmd::init::run_auto(
            tmp.path().to_str().unwrap(),
            Some(case.name),
            Some("team"),
            Some("claude"),
            Some("minimal"),
            false,
            false,
        )
        .unwrap();

        let project =
            hlv::model::project::ProjectMap::load(&tmp.path().join(".hlv/project.yaml")).unwrap();
        let code = project.paths.code.as_ref().unwrap();
        let expected_source_roots = case
            .source_roots
            .iter()
            .map(|root| root.to_string())
            .collect::<Vec<_>>();
        let expected_test_roots = case
            .test_roots
            .iter()
            .map(|root| root.to_string())
            .collect::<Vec<_>>();
        assert_eq!(code.src, expected_source_roots);
        assert_eq!(code.tests.clone().unwrap_or_default(), expected_test_roots);
        let stack = project.stack.as_ref().unwrap();
        let languages = stack
            .components
            .iter()
            .flat_map(|component| component.languages.iter().cloned())
            .collect::<Vec<_>>();
        let expected_languages = case
            .languages
            .iter()
            .map(|language| language.to_string())
            .collect::<Vec<_>>();
        assert_eq!(languages, expected_languages);

        let gates = hlv::model::policy::GatesPolicy::load(
            &tmp.path().join(".hlv/validation/gates-policy.yaml"),
        )
        .unwrap();
        let gate = gates
            .gates
            .iter()
            .find(|gate| gate.id == "GATE-CONTRACT-001")
            .unwrap();
        assert_eq!(gate.command.as_deref(), Some(case.command));
        assert_eq!(gate.cwd.as_deref(), Some("."));
    }
}

#[test]
fn init_adopt_builds_initial_index_and_seeds_map() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("Cargo.toml"), "[package]\nname = \"app\"\n").unwrap();
    fs::create_dir_all(tmp.path().join("src")).unwrap();
    fs::write(tmp.path().join("src/lib.rs"), "pub fn real() {}\n").unwrap();

    hlv::cmd::init::run_auto(
        tmp.path().to_str().unwrap(),
        Some("indexed-adopt"),
        Some("team"),
        Some("claude"),
        Some("minimal"),
        false,
        false,
    )
    .unwrap();

    let index =
        hlv::model::index::Index::load(&tmp.path().join(".hlv/index/signatures.yaml")).unwrap();
    let symbol = index
        .symbols
        .iter()
        .find(|symbol| symbol.name == "real")
        .expect("initial index should include source symbol");

    let map = hlv::model::llm_map::LlmMap::load(&tmp.path().join(".hlv/llm/map.yaml")).unwrap();
    assert_eq!(map.entries.len(), 1);
    assert_eq!(map.entries[0].path, "src/");
    assert_eq!(map.entries[0].layer, "code");
    assert_eq!(
        map.entries[0].index_ref.as_deref(),
        Some(symbol.id.as_str())
    );
}

#[test]
fn cli_adopt_init_check_and_index_commands_work() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("Cargo.toml"), "[package]\nname = \"app\"\n").unwrap();
    fs::create_dir_all(tmp.path().join("src")).unwrap();
    fs::write(
        tmp.path().join("src/lib.rs"),
        "pub fn real() -> &'static str { \"ok\" }\n",
    )
    .unwrap();

    let init = Command::new(hlv_binary())
        .args([
            "init",
            "--adopt",
            "--path",
            tmp.path().to_str().unwrap(),
            "--project",
            "cli-adopt",
            "--owner",
            "team",
            "--agent",
            "claude",
            "--profile",
            "minimal",
        ])
        .output()
        .expect("run init");
    assert!(
        init.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    let init_stdout = String::from_utf8_lossy(&init.stdout);
    assert!(init_stdout.contains("Project scaffold created"));
    assert!(!init_stdout.contains("pub fn real"));
    assert!(tmp.path().join(".hlv/project.yaml").exists());
    assert!(!tmp.path().join("project.yaml").exists());

    let check = Command::new(hlv_binary())
        .args(["--root", tmp.path().to_str().unwrap(), "check"])
        .output()
        .expect("run check");
    assert!(
        check.status.success(),
        "check failed: stdout={} stderr={}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );

    let build = Command::new(hlv_binary())
        .args(["--root", tmp.path().to_str().unwrap(), "index", "build"])
        .output()
        .expect("run index build");
    assert!(
        build.status.success(),
        "index build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let build_stdout = String::from_utf8_lossy(&build.stdout);
    assert!(build_stdout.contains("indexed"));
    assert!(!build_stdout.contains("pub fn real"));

    let show = Command::new(hlv_binary())
        .args([
            "--root",
            tmp.path().to_str().unwrap(),
            "index",
            "show",
            "real",
            "--json",
        ])
        .output()
        .expect("run index show");
    assert!(
        show.status.success(),
        "index show failed: {}",
        String::from_utf8_lossy(&show.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(json[0]["name"], "real");
}

#[test]
fn cli_check_example_project_still_passes() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/example-project");
    let output = Command::new(hlv_binary())
        .args(["--root", fixture.to_str().unwrap(), "check"])
        .output()
        .expect("run check");

    assert!(
        output.status.success(),
        "example project check failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn adopt_init_generates_adopt_aware_hlv_md() {
    let tmp = TempDir::new().unwrap();

    hlv::cmd::init::run_with_options(
        tmp.path().to_str().unwrap(),
        Some("adopted-init"),
        Some("team"),
        Some("claude"),
        Some("adopt-test"),
        Some("minimal"),
        true,
    )
    .unwrap();

    let hlv_md = fs::read_to_string(tmp.path().join("HLV.md")).unwrap();
    assert!(hlv_md.contains("`.hlv/project.yaml`"));
    assert!(hlv_md.contains("HLV adopt mode"));
    assert!(hlv_md.contains("paths.code"));
    assert!(hlv_md.contains("Legacy code is observed in place"));
}

#[test]
fn adopt_reinit_updates_hlv_layout_in_place() {
    let tmp = TempDir::new().unwrap();
    hlv::cmd::init::run_with_options(
        tmp.path().to_str().unwrap(),
        Some("adopted-init"),
        Some("team"),
        Some("claude"),
        Some("adopt-test"),
        Some("minimal"),
        true,
    )
    .unwrap();

    // Reinit (adopt flag not required — layout is auto-detected)
    hlv::cmd::init::run_with_options(
        tmp.path().to_str().unwrap(),
        None,
        None,
        None,
        None,
        None,
        false,
    )
    .unwrap();

    assert!(tmp.path().join(".hlv/project.yaml").exists());
    assert!(!tmp.path().join("project.yaml").exists());
}

#[test]
fn adopt_init_schema_comments_resolve_from_hlv_layout() {
    let tmp = TempDir::new().unwrap();
    hlv::cmd::init::run_with_options(
        tmp.path().to_str().unwrap(),
        Some("adopted-init"),
        Some("team"),
        Some("claude"),
        Some("adopt-test"),
        Some("minimal"),
        true,
    )
    .unwrap();

    // Every generated YAML with a $schema comment must point at an existing
    // schema file when resolved relative to the YAML's own directory.
    let mut checked = 0;
    let mut stack = vec![tmp.path().join(".hlv")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "yaml") {
                continue;
            }
            let content = fs::read_to_string(&path).unwrap();
            let Some(line) = content
                .lines()
                .find(|l| l.contains("yaml-language-server: $schema="))
            else {
                continue;
            };
            let rel = line.split("$schema=").nth(1).unwrap().trim();
            let resolved = path.parent().unwrap().join(rel);
            assert!(
                resolved.exists(),
                "$schema target missing for {}: {rel}",
                path.display()
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 10,
        "expected many schema comments, got {checked}"
    );
}

#[test]
fn greenfield_layout_still_prioritized_over_hlv_dir() {
    let tmp = TempDir::new().unwrap();
    write_minimal_adopted_project(tmp.path());
    // Add a root-level project.yaml — root layout must win
    fs::write(
        tmp.path().join("project.yaml"),
        fs::read(tmp.path().join(".hlv/project.yaml")).unwrap(),
    )
    .unwrap();

    assert_eq!(hlv::config_root(tmp.path()), tmp.path());
}

// ═══════════════════════════════════════════════════════
// Legacy changed-file scope (LEG-010)
// ═══════════════════════════════════════════════════════

fn write_legacy_project_yaml(root: &Path, base_ref: Option<&str>) {
    write_legacy_project_yaml_with_markers(root, base_ref, true, true);
}

fn write_legacy_project_yaml_with_markers(
    root: &Path,
    base_ref: Option<&str>,
    hlv_markers: bool,
    security_markers: bool,
) {
    let base_ref_line = base_ref
        .map(|base| format!("  base_ref: {base}\n"))
        .unwrap_or_default();
    fs::write(
        root.join(".hlv/project.yaml"),
        format!(
            r#"schema_version: 1
project: adopted-demo
status: draft
hlv_root: .hlv
paths:
  human:
    glossary: human/glossary.yaml
    constraints: human/constraints/
  validation:
    gates_policy: validation/gates-policy.yaml
    scenarios: validation/scenarios/
  llm:
    src: llm/src/
    tests: llm/tests/
    map: llm/map.yaml
  code:
    src: [app/]
features:
  legacy_mode: true
  hlv_markers: {hlv_markers}
  security_markers: {security_markers}
git:
  commit_convention: conventional
  merge_strategy: manual
{base_ref_line}"#
        ),
    )
    .unwrap();
}

fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_committed_git_repo(dir: &Path) {
    git(dir, &["init", "-b", "main"]);
    git(dir, &["config", "user.email", "test@test.com"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-m", "init"]);
}

fn leg010_diags(root: &Path) -> Vec<String> {
    let report = get_check_report(root, CheckOptions::default()).unwrap();
    report
        .diagnostics
        .iter()
        .filter(|d| d.code == "LEG-010")
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn legacy_scope_does_not_warn_when_marker_validation_disabled() {
    let tmp = TempDir::new().unwrap();
    write_minimal_adopted_project(tmp.path());
    write_legacy_project_yaml_with_markers(tmp.path(), None, false, false);

    let warnings = leg010_diags(tmp.path());
    assert!(
        warnings.is_empty(),
        "marker-disabled adopted projects should not need legacy scope: {warnings:?}"
    );
}

#[test]
fn legacy_scope_warns_when_undetectable() {
    let tmp = TempDir::new().unwrap();
    write_minimal_adopted_project(tmp.path());
    write_legacy_project_yaml(tmp.path(), None);
    init_committed_git_repo(tmp.path());

    // Clean committed worktree, no changed_files, no base_ref: the changed
    // file set cannot be determined and must warn instead of silently
    // skipping marker checks (the CI case).
    let warnings = leg010_diags(tmp.path());
    assert!(
        !warnings.is_empty(),
        "expected LEG-010 when scope is undetectable"
    );
}

#[test]
fn legacy_scope_with_base_ref_is_deterministic() {
    let tmp = TempDir::new().unwrap();
    write_minimal_adopted_project(tmp.path());
    write_legacy_project_yaml(tmp.path(), Some("main"));
    init_committed_git_repo(tmp.path());

    // With a resolvable base ref the (possibly empty) merge-base diff is a
    // legitimate deterministic scope — no warning even on a clean worktree.
    let warnings = leg010_diags(tmp.path());
    assert!(
        warnings.is_empty(),
        "base_ref scope should not warn: {warnings:?}"
    );

    // Committed milestone work on a branch is still in scope (no warning,
    // diff against merge-base picks it up even with a clean worktree).
    git(tmp.path(), &["checkout", "-b", "feature"]);
    fs::write(
        tmp.path().join("app/main.py"),
        "def main():\n    return 1\n",
    )
    .unwrap();
    git(tmp.path(), &["add", "."]);
    git(tmp.path(), &["commit", "-m", "change"]);
    let warnings = leg010_diags(tmp.path());
    assert!(
        warnings.is_empty(),
        "committed branch changes should resolve scope: {warnings:?}"
    );
}

#[test]
fn legacy_scope_warns_on_unresolvable_base_ref() {
    let tmp = TempDir::new().unwrap();
    write_minimal_adopted_project(tmp.path());
    write_legacy_project_yaml(tmp.path(), Some("origin/does-not-exist"));
    init_committed_git_repo(tmp.path());

    let warnings = leg010_diags(tmp.path());
    assert!(
        warnings.iter().any(|m| m.contains("origin/does-not-exist")),
        "expected LEG-010 naming the unresolvable base ref: {warnings:?}"
    );
}
