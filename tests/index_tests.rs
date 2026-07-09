use std::fs;
use std::path::Path;

use tempfile::TempDir;

use hlv::cmd::index::{find_symbols, list_symbols_by_file};
use hlv::index::builder::build_index;
use hlv::model::index::Index;

fn write_adopted_project(root: &Path) {
    fs::create_dir_all(root.join(".hlv/human/constraints")).unwrap();
    fs::create_dir_all(root.join(".hlv/validation/scenarios")).unwrap();
    fs::create_dir_all(root.join(".hlv/llm")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join(".hlv/human/glossary.yaml"),
        "schema_version: 1\ntypes: {}\nenums: {}\nterms: {}\nrules: []\n",
    )
    .unwrap();
    fs::write(
        root.join(".hlv/validation/gates-policy.yaml"),
        "version: 1.0.0\npolicy_id: TEST\ngates: []\n",
    )
    .unwrap();
    fs::write(
        root.join(".hlv/llm/map.yaml"),
        "schema_version: 1\nentries: []\n",
    )
    .unwrap();
    fs::write(
        root.join(".hlv/milestones.yaml"),
        "project: adopted-index\nhistory: []\n",
    )
    .unwrap();
    fs::write(
        root.join(".hlv/project.yaml"),
        r#"schema_version: 1
project: adopted-index
status: draft
hlv_root: .hlv
paths:
  human:
    glossary: human/glossary.yaml
    constraints: human/constraints/
  validation:
    scenarios: validation/scenarios/
    gates_policy: validation/gates-policy.yaml
  llm:
    src: llm/src/
    map: llm/map.yaml
  code:
    src: [src/]
features:
  legacy_mode: true
  hlv_markers: false
  security_markers: false
"#,
    )
    .unwrap();
}

#[test]
fn index_build_writes_signatures_for_adopted_rust_project() {
    let tmp = TempDir::new().unwrap();
    write_adopted_project(tmp.path());
    fs::write(
        tmp.path().join("src/lib.rs"),
        "pub struct User;\npub fn greeting(name: &str) -> String { name.to_string() }\n",
    )
    .unwrap();

    let summary = build_index(tmp.path()).unwrap();
    assert_eq!(summary.files_scanned, 1);
    assert!(summary.symbols_indexed >= 2);
    assert_eq!(
        summary.output,
        tmp.path().join(".hlv/index/signatures.yaml")
    );

    let index = Index::load(&summary.output).unwrap();
    assert!(index.symbols.iter().any(|symbol| symbol.name == "User"));
    assert!(index.symbols.iter().any(|symbol| symbol.name == "greeting"));
}

#[test]
fn index_build_ignores_nested_build_directories() {
    let tmp = TempDir::new().unwrap();
    write_adopted_project(tmp.path());
    fs::write(tmp.path().join("src/lib.rs"), "pub fn real() {}\n").unwrap();
    fs::create_dir_all(tmp.path().join("src/target")).unwrap();
    fs::write(
        tmp.path().join("src/target/generated.rs"),
        "pub fn ignored() {}\n",
    )
    .unwrap();

    let summary = build_index(tmp.path()).unwrap();
    let index = Index::load(&summary.output).unwrap();
    assert!(index.symbols.iter().any(|symbol| symbol.name == "real"));
    assert!(
        !index.symbols.iter().any(|symbol| symbol.name == "ignored"),
        "target/ should be ignored"
    );
}

#[test]
fn index_show_finds_symbol_by_name_and_id() {
    let tmp = TempDir::new().unwrap();
    write_adopted_project(tmp.path());
    fs::write(tmp.path().join("src/lib.rs"), "pub fn real() {}\n").unwrap();
    let summary = build_index(tmp.path()).unwrap();
    let index = Index::load(&summary.output).unwrap();
    let id = index
        .symbols
        .iter()
        .find(|symbol| symbol.name == "real")
        .unwrap()
        .id
        .clone();

    let by_name = find_symbols(tmp.path(), "real").unwrap();
    let by_id = find_symbols(tmp.path(), &id).unwrap();

    assert_eq!(by_name.len(), 1);
    assert_eq!(by_id.len(), 1);
    assert_eq!(by_name[0].id, id);
}

#[test]
fn index_list_filters_by_file() {
    let tmp = TempDir::new().unwrap();
    write_adopted_project(tmp.path());
    fs::write(tmp.path().join("src/lib.rs"), "pub fn real() {}\n").unwrap();
    fs::write(tmp.path().join("src/other.rs"), "pub fn other() {}\n").unwrap();
    build_index(tmp.path()).unwrap();

    let symbols = list_symbols_by_file(tmp.path(), "src/lib.rs").unwrap();
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "real");
}

#[test]
fn index_query_json_shape_is_stable() {
    let tmp = TempDir::new().unwrap();
    write_adopted_project(tmp.path());
    fs::write(tmp.path().join("src/lib.rs"), "pub fn real() {}\n").unwrap();
    build_index(tmp.path()).unwrap();

    let symbols = find_symbols(tmp.path(), "real").unwrap();
    let json = serde_json::to_value(&symbols).unwrap();
    let first = &json.as_array().unwrap()[0];
    assert_eq!(first["name"], "real");
    assert_eq!(first["file"], "src/lib.rs");
    assert!(first.get("signature").is_some());
    assert!(first.get("source_fingerprint").is_some());
}
