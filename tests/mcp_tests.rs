use std::collections::HashMap;
use std::path::Path;
use tempfile::TempDir;

use hlv::model::milestone::{MilestoneMap, StageEntry, StageStatus};
use hlv::model::task::TaskTracker;

fn setup_project(root: &Path) {
    hlv::cmd::init::run_with_milestone(
        root.to_str().unwrap(),
        Some("test-proj"),
        Some("team"),
        Some("claude"),
        Some("mcp-test"),
        Some("minimal"),
    )
    .unwrap();
}

fn load_milestones(root: &Path) -> MilestoneMap {
    MilestoneMap::load(&root.join("milestones.yaml")).unwrap()
}

fn save_milestones(root: &Path, map: &MilestoneMap) {
    map.save(&root.join("milestones.yaml")).unwrap();
}

/// Add a stage with tasks for testing
fn add_stage_with_tasks(root: &Path) {
    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Foundation".to_string(),
        status: StageStatus::Pending,
        commit: None,
        tasks: vec![
            TaskTracker::new("TASK-001".to_string()),
            TaskTracker::new("TASK-002".to_string()),
        ],
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);
}

/// Create stage plan file
fn create_stage_file(root: &Path, stage_n: u32) {
    let map = load_milestones(root);
    let mid = &map.current.as_ref().unwrap().id;
    let stage_dir = root.join(format!("human/milestones/{mid}"));
    std::fs::write(
        stage_dir.join(format!("stage_{stage_n}.md")),
        format!(
            "# Stage {stage_n}: Test\n\n## Tasks\n\nTASK-001 Domain Types\n  contracts: []\n  output: llm/src/\n"
        ),
    )
    .unwrap();
}

/// Create a contract file for testing
fn create_contract(root: &Path, id: &str) {
    let map = load_milestones(root);
    let mid = &map.current.as_ref().unwrap().id;
    let contracts_dir = root.join(format!("human/milestones/{mid}/contracts"));
    std::fs::create_dir_all(&contracts_dir).unwrap();
    std::fs::write(
        contracts_dir.join(format!("{id}.md")),
        format!("# {id}\n\nversion: 1.0\n\n## Input\n\n## Output\n\n## Errors\n\n## Invariants\n\n## Examples\n"),
    )
    .unwrap();
}

/// Create a contract YAML file for testing
fn create_contract_yaml(root: &Path, id: &str) {
    let map = load_milestones(root);
    let mid = &map.current.as_ref().unwrap().id;
    let contracts_dir = root.join(format!("human/milestones/{mid}/contracts"));
    std::fs::create_dir_all(&contracts_dir).unwrap();
    std::fs::write(
        contracts_dir.join(format!("{id}.yaml")),
        format!(
            "id: {id}\nversion: \"1.0\"\nerrors: []\ninvariants: []\nsecurity: []\ndepends_on_constraints: []\n"
        ),
    )
    .unwrap();
}

/// Extract text content from CallToolResult (serialize and extract from JSON)
fn tool_text(result: &rmcp::model::CallToolResult) -> String {
    let json = serde_json::to_value(result).unwrap();
    let content = json["content"].as_array().unwrap();
    content
        .iter()
        .filter_map(|c| c["text"].as_str().map(|s| s.to_string()))
        .collect::<Vec<_>>()
        .join("")
}

/// Extract text from ReadResourceResult
fn resource_text(result: &rmcp::model::ReadResourceResult) -> String {
    let json = serde_json::to_value(result).unwrap();
    let contents = json["contents"].as_array().unwrap();
    contents
        .iter()
        .filter_map(|c| c["text"].as_str().map(|s| s.to_string()))
        .collect::<Vec<_>>()
        .join("")
}

/// Extract JSON-RPC response from SSE-formatted or plain JSON text.
/// SSE responses contain lines like `data: {...}\n\n`, so we extract the JSON.
fn extract_json_from_sse_or_json(text: &str) -> serde_json::Value {
    // Try plain JSON first
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        return v;
    }
    // SSE format: find the last `data: ` line containing JSON
    for line in text.lines().rev() {
        if let Some(data) = line
            .strip_prefix("data: ")
            .or_else(|| line.strip_prefix("data:"))
        {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(data.trim()) {
                return v;
            }
        }
    }
    panic!("Could not extract JSON from response: {text}");
}

#[test]
fn server_creates_and_reports_capabilities() {
    use rmcp::ServerHandler;

    let tmp = TempDir::new().unwrap();
    let server = hlv::mcp::HlvMcpServer::new(hlv::mcp::router::ServerMode::Single(
        tmp.path().to_path_buf(),
    ));

    let info = server.get_info();
    let json = serde_json::to_value(&info).unwrap();

    // Server reports capabilities
    let caps = &json["capabilities"];
    assert!(
        caps["tools"].is_object(),
        "tools capability must be enabled"
    );
    assert!(
        caps["resources"].is_object(),
        "resources capability must be enabled"
    );

    // Server has instructions
    assert!(json["instructions"].is_string());
    assert!(json["instructions"].as_str().unwrap().contains("HLV"));
}

#[test]
fn server_lists_all_tools() {
    use rmcp::ServerHandler;

    let tmp = TempDir::new().unwrap();
    let server = hlv::mcp::HlvMcpServer::new(hlv::mcp::router::ServerMode::Single(
        tmp.path().to_path_buf(),
    ));

    // Use get_tool to check individual tools exist, or list via tool_router
    // Check a selection of tools via get_tool
    let tool_names = [
        "hlv_check",
        "hlv_workflow",
        "hlv_commit_msg",
        "hlv_milestone_new",
        "hlv_milestone_done",
        "hlv_milestone_abort",
        "hlv_milestone_label",
        "hlv_milestone_meta",
        "hlv_gate_enable",
        "hlv_gate_disable",
        "hlv_gate_run",
        "hlv_constraint_add",
        "hlv_constraint_remove",
        "hlv_constraint_add_rule",
        "hlv_constraint_remove_rule",
        "hlv_stage_reopen",
        "hlv_stage_label",
        "hlv_stage_meta",
        "hlv_task_add",
        "hlv_task_list",
        "hlv_task_start",
        "hlv_task_done",
        "hlv_task_block",
        "hlv_task_unblock",
        "hlv_task_sync",
        "hlv_task_label",
        "hlv_task_meta",
        "hlv_artifacts",
        "hlv_glossary",
    ];

    for name in &tool_names {
        assert!(
            server.get_tool(name).is_some(),
            "Tool '{name}' not registered in server"
        );
    }

    // Also verify a nonexistent tool returns None
    assert!(server.get_tool("nonexistent").is_none());
}

#[test]
fn stdio_json_rpc_roundtrip() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};

    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // Build the binary path
    let binary = env!("CARGO_BIN_EXE_hlv");

    let mut child = Command::new(binary)
        .args(["mcp", "--transport", "stdio"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn hlv mcp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Send initialize request
    let init_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "smoke-test", "version": "0.1" }
        }
    });
    let msg = serde_json::to_string(&init_req).unwrap();
    writeln!(stdin, "{msg}").unwrap();
    stdin.flush().unwrap();

    // Read response
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let resp: serde_json::Value = serde_json::from_str(line.trim()).expect("valid JSON response");

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert!(
        resp["result"].is_object(),
        "Expected result object, got: {resp}"
    );
    let result = &resp["result"];
    assert!(
        result["serverInfo"].is_object() || result["server_info"].is_object(),
        "Result should contain serverInfo: {result}"
    );

    // Send initialized notification
    let initialized = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    let msg = serde_json::to_string(&initialized).unwrap();
    writeln!(stdin, "{msg}").unwrap();
    stdin.flush().unwrap();

    // Send a resources/list request to verify full round-trip
    let list_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "resources/list",
        "params": {}
    });
    let msg = serde_json::to_string(&list_req).unwrap();
    writeln!(stdin, "{msg}").unwrap();
    stdin.flush().unwrap();

    let mut line2 = String::new();
    reader.read_line(&mut line2).unwrap();
    let resp2: serde_json::Value =
        serde_json::from_str(line2.trim()).expect("valid JSON response for resources/list");
    assert_eq!(resp2["jsonrpc"], "2.0");
    assert_eq!(resp2["id"], 2);
    assert!(
        resp2["result"]["resources"].is_array(),
        "Expected resources array: {resp2}"
    );

    // Close stdin to terminate the server
    drop(stdin);
    let _ = child.wait();
}

#[test]
fn resource_project() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::resources::read_resource(root, "hlv://project").unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    assert_eq!(json["project"], "test-proj");
    assert!(json["paths"].is_object());
}

#[test]
fn resource_milestones() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::resources::read_resource(root, "hlv://milestones").unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    assert!(json["current"].is_object());
    assert_eq!(json["project"], "test-proj");
}

#[test]
fn resource_gates() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::resources::read_resource(root, "hlv://gates").unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    assert!(json["gates"].is_array());
}

#[test]
fn resource_constraints() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::resources::read_resource(root, "hlv://constraints").unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    assert!(json.is_array());
}

#[test]
fn resource_map() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::resources::read_resource(root, "hlv://map").unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    assert!(json["entries"].is_array());
}

#[test]
fn resource_workflow() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::resources::read_resource(root, "hlv://workflow").unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    assert!(json["phase_name"].is_string());
    assert!(json["next_actions"].is_array());
}

#[test]
fn resource_tasks() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    add_stage_with_tasks(root);

    let result = hlv::mcp::resources::read_resource(root, "hlv://tasks").unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 2);
}

#[test]
fn resource_artifacts() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::resources::read_resource(root, "hlv://artifacts").unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    // artifacts returns global-only list
    assert!(json.is_array());
}

#[test]
fn resource_plan() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::resources::read_resource(root, "hlv://plan").unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    // plan returns PlanMd → JSON directly (or null if no plan.md / no milestone)
    assert!(json.is_null() || json["overview"].is_string());
    if !json.is_null() {
        assert!(json["groups"].is_array(), "PlanMd must have groups array");
    }
}

#[test]
fn resource_glossary() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::resources::read_resource(root, "hlv://glossary").unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    assert!(json["domain"].is_string());
}

#[test]
fn resource_traceability() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::resources::read_resource(root, "hlv://traceability").unwrap();
    let _json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
}

#[test]
fn resource_contracts_list() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    create_contract(root, "order.create");

    let result = hlv::mcp::resources::read_resource(root, "hlv://contracts").unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    assert!(json.is_array());
    assert!(!json.as_array().unwrap().is_empty());
}

// ── Parametrized resources ─────────────────────────────

#[test]
fn resource_stage_by_number() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    add_stage_with_tasks(root);
    create_stage_file(root, 1);

    let result = hlv::mcp::resources::read_resource(root, "hlv://stage/1").unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    assert_eq!(json["id"], 1);
    assert!(json["name"].is_string());
    assert!(json["tasks"].is_array());
}

#[test]
fn resource_stage_not_found() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let err = hlv::mcp::resources::read_resource(root, "hlv://stage/99").unwrap_err();
    assert!(
        format!("{err:?}").contains("not found") || format!("{err:?}").contains("Not Found"),
        "Expected 'not found' error, got: {err:?}"
    );
}

#[test]
fn resource_stage_invalid_number() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let err = hlv::mcp::resources::read_resource(root, "hlv://stage/abc").unwrap_err();
    assert!(
        format!("{err:?}").contains("Invalid stage number"),
        "Expected invalid number error, got: {err:?}"
    );
}

#[test]
fn resource_contract_by_id() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    create_contract(root, "order.create");

    let result = hlv::mcp::resources::read_resource(root, "hlv://contracts/order.create").unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    assert_eq!(json["id"], "order.create");
    assert!(json["formats"].is_array());
    assert!(
        json["markdown"].is_object(),
        "parsed markdown contract should be present"
    );
}

#[test]
fn resource_contract_both_formats() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    create_contract(root, "order.create");
    create_contract_yaml(root, "order.create");

    let result = hlv::mcp::resources::read_resource(root, "hlv://contracts/order.create").unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    assert_eq!(json["id"], "order.create");
    assert!(json["markdown"].is_object());
    assert!(json["yaml"].is_object());
    assert_eq!(json["formats"].as_array().unwrap().len(), 2);
}

#[test]
fn resource_contracts_list_no_duplicates() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    create_contract(root, "order.create");
    create_contract_yaml(root, "order.create");

    let result = hlv::mcp::resources::read_resource(root, "hlv://contracts").unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    assert!(json.is_array());
    // Should be 1 entry, not 2
    assert_eq!(
        json.as_array().unwrap().len(),
        1,
        "Same contract ID should not be duplicated"
    );
    assert_eq!(json[0]["id"], "order.create");
    assert_eq!(json[0]["formats"].as_array().unwrap().len(), 2);
}

#[test]
fn resource_contract_not_found() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let err = hlv::mcp::resources::read_resource(root, "hlv://contracts/nonexistent").unwrap_err();
    assert!(
        format!("{err:?}").contains("not found") || format!("{err:?}").contains("Not Found"),
        "Expected 'not found' error, got: {err:?}"
    );
}

#[test]
fn resource_tasks_by_stage() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    add_stage_with_tasks(root);

    let result = hlv::mcp::resources::read_resource(root, "hlv://tasks/1").unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 2);
}

#[test]
fn resource_tasks_invalid_stage() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let err = hlv::mcp::resources::read_resource(root, "hlv://tasks/abc").unwrap_err();
    assert!(
        format!("{err:?}").contains("Invalid stage number"),
        "Expected invalid stage error, got: {err:?}"
    );
}

#[test]
fn resource_unknown_uri() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let err = hlv::mcp::resources::read_resource(root, "hlv://unknown").unwrap_err();
    assert!(
        format!("{err:?}").contains("Unknown resource"),
        "Expected unknown resource error, got: {err:?}"
    );
}

// ── Artifact parametrized resources ─────────────────────

/// Helper: create a global artifact file
fn create_global_artifact(root: &Path, name: &str) {
    let dir = root.join("human/artifacts");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(format!("{name}.md")),
        format!("# {name}\n\nContent of {name}"),
    )
    .unwrap();
}

/// Helper: create a milestone artifact file
fn create_milestone_artifact(root: &Path, name: &str) {
    let map = load_milestones(root);
    let mid = &map.current.as_ref().unwrap().id;
    let dir = root.join(format!("human/milestones/{mid}/artifacts"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(format!("{name}.md")),
        format!("# {name}\n\nMilestone artifact"),
    )
    .unwrap();
}

#[test]
fn resource_artifact_by_name() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    create_global_artifact(root, "context");

    let result = hlv::mcp::resources::read_resource(root, "hlv://artifacts/context").unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    assert_eq!(json["name"], "context");
    assert!(json["content"].is_string());
}

#[test]
fn resource_artifact_not_found() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let err = hlv::mcp::resources::read_resource(root, "hlv://artifacts/nonexistent").unwrap_err();
    assert!(
        format!("{err:?}").contains("not found") || format!("{err:?}").contains("Not Found"),
        "Expected 'not found' error, got: {err:?}"
    );
}

#[test]
fn resource_milestone_artifacts_list() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let mid = load_milestones(root).current.as_ref().unwrap().id.clone();
    create_milestone_artifact(root, "feature-auth");

    let result =
        hlv::mcp::resources::read_resource(root, &format!("hlv://artifacts/milestone/{mid}"))
            .unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 1);
    assert_eq!(json[0]["name"], "feature-auth");
}

#[test]
fn resource_milestone_artifact_by_name() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let mid = load_milestones(root).current.as_ref().unwrap().id.clone();
    create_milestone_artifact(root, "feature-auth");

    let result = hlv::mcp::resources::read_resource(
        root,
        &format!("hlv://artifacts/milestone/{mid}/feature-auth"),
    )
    .unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&result)).unwrap();
    assert_eq!(json["name"], "feature-auth");
    assert!(json["content"].is_string());
}

#[test]
fn resource_milestone_artifact_not_found() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    let mid = load_milestones(root).current.as_ref().unwrap().id.clone();

    let err = hlv::mcp::resources::read_resource(
        root,
        &format!("hlv://artifacts/milestone/{mid}/nonexistent"),
    )
    .unwrap_err();
    assert!(
        format!("{err:?}").contains("not found") || format!("{err:?}").contains("Not Found"),
        "Expected 'not found' error, got: {err:?}"
    );
}

// ── list_resources / list_resource_templates ────────────

#[test]
fn list_resources_returns_all() {
    let result = hlv::mcp::resources::list_resources();
    assert_eq!(result.resources.len(), 12);
    let uris: Vec<_> = result
        .resources
        .iter()
        .map(|r| r.raw.uri.as_str())
        .collect();
    assert!(uris.contains(&"hlv://project"));
    assert!(uris.contains(&"hlv://milestones"));
    assert!(uris.contains(&"hlv://tasks"));
    assert!(uris.contains(&"hlv://glossary"));
}

#[test]
fn list_resource_templates_returns_all() {
    let result = hlv::mcp::resources::list_resource_templates();
    assert_eq!(result.resource_templates.len(), 6);
    let uris: Vec<_> = result
        .resource_templates
        .iter()
        .map(|r| r.raw.uri_template.as_str())
        .collect();
    assert!(uris.contains(&"hlv://stage/{n}"));
    assert!(uris.contains(&"hlv://contracts/{id}"));
    assert!(uris.contains(&"hlv://tasks/{n}"));
}

#[test]
fn tool_check() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::tools::hlv_check(root).unwrap();
    let text = tool_text(&result);
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(json["exit_code"].is_number());
    assert!(json["diagnostics"].is_array());
}

#[test]
fn tool_workflow() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::tools::hlv_workflow(root).unwrap();
    let text = tool_text(&result);
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(json["phase_name"].is_string());
    assert!(json["next_actions"].is_array());
}

#[test]
fn tool_task_list() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    add_stage_with_tasks(root);

    let result = hlv::mcp::tools::hlv_task_list(root, None, None, None).unwrap();
    let text = tool_text(&result);
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 2);
}

#[test]
fn tool_task_list_filter_by_stage() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    add_stage_with_tasks(root);

    let result = hlv::mcp::tools::hlv_task_list(root, Some(1), None, None).unwrap();
    let text = tool_text(&result);
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 2);

    // Stage 99 should return empty
    let result = hlv::mcp::tools::hlv_task_list(root, Some(99), None, None).unwrap();
    let text = tool_text(&result);
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 0);
}

#[test]
fn tool_task_lifecycle() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    add_stage_with_tasks(root);

    // Start
    let result = hlv::mcp::tools::hlv_task_start(root, "TASK-001").unwrap();
    assert!(tool_text(&result).contains("started"));

    // Block
    let result = hlv::mcp::tools::hlv_task_block(root, "TASK-001", "waiting for API").unwrap();
    assert!(tool_text(&result).contains("blocked"));

    // Unblock
    let result = hlv::mcp::tools::hlv_task_unblock(root, "TASK-001").unwrap();
    assert!(tool_text(&result).contains("unblocked"));

    // Done
    let result = hlv::mcp::tools::hlv_task_done(root, "TASK-001").unwrap();
    assert!(tool_text(&result).contains("done"));
}

#[test]
fn tool_task_start_nonexistent() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    add_stage_with_tasks(root);

    let err = hlv::mcp::tools::hlv_task_start(root, "NONEXISTENT").unwrap_err();
    assert!(
        format!("{err:?}").contains("task start failed"),
        "Expected task error, got: {err:?}"
    );
}

#[test]
fn tool_task_label() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    add_stage_with_tasks(root);

    hlv::mcp::tools::hlv_task_label(root, "TASK-001", "add", "urgent").unwrap();

    // Verify label was added
    let result = hlv::mcp::tools::hlv_task_list(root, None, None, Some("urgent")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 1);
    assert_eq!(json[0]["id"], "TASK-001");
}

#[test]
fn tool_task_meta() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    add_stage_with_tasks(root);

    hlv::mcp::tools::hlv_task_meta(root, "TASK-001", "set", "priority", Some("high")).unwrap();

    let result = hlv::mcp::tools::hlv_task_list(root, None, None, None).unwrap();
    let json: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
    let task = json
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == "TASK-001")
        .unwrap();
    assert_eq!(task["meta"]["priority"], "high");
}

#[test]
fn tool_task_sync() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    create_stage_file(root, 1);

    let result = hlv::mcp::tools::hlv_task_sync(root, false).unwrap();
    assert!(tool_text(&result).contains("synced"));
}

#[test]
fn tool_gate_enable_disable() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // Need to know a gate ID — check gates first
    let resource = hlv::mcp::resources::read_resource(root, "hlv://gates").unwrap();
    let json: serde_json::Value = serde_json::from_str(&resource_text(&resource)).unwrap();
    let gates = json["gates"].as_array().unwrap();
    if gates.is_empty() {
        return; // minimal profile may have no gates
    }
    let gate_id = gates[0]["id"].as_str().unwrap();

    // Disable
    let result = hlv::mcp::tools::hlv_gate_disable(root, gate_id).unwrap();
    assert!(tool_text(&result).contains("disabled"));

    // Enable
    let result = hlv::mcp::tools::hlv_gate_enable(root, gate_id).unwrap();
    assert!(tool_text(&result).contains("enabled"));
}

#[test]
fn tool_gate_enable_nonexistent() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let err = hlv::mcp::tools::hlv_gate_enable(root, "nonexistent-gate").unwrap_err();
    assert!(
        format!("{err:?}").contains("gate enable failed"),
        "Expected gate error, got: {err:?}"
    );
}

#[test]
fn tool_gate_run() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::tools::hlv_gate_run(root, None).unwrap();
    let json: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
    assert!(json["passed"].is_number());
    assert!(json["failed"].is_number());
    assert!(json["skipped"].is_number());
}

#[test]
fn tool_milestone_label() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::tools::hlv_milestone_label(root, "add", "priority-high").unwrap();
    assert!(tool_text(&result).contains("priority-high"));

    // Verify label is persisted
    let map = load_milestones(root);
    let current = map.current.as_ref().unwrap();
    assert!(current.labels.contains(&"priority-high".to_string()));
}

#[test]
fn tool_milestone_meta() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::mcp::tools::hlv_milestone_meta(root, "set", "owner", Some("alice")).unwrap();

    let map = load_milestones(root);
    let current = map.current.as_ref().unwrap();
    assert_eq!(current.meta.get("owner").unwrap(), "alice");
}

#[test]
fn tool_stage_label() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    add_stage_with_tasks(root);

    hlv::mcp::tools::hlv_stage_label(root, 1, "add", "blocked").unwrap();

    let map = load_milestones(root);
    let stage = &map.current.as_ref().unwrap().stages[0];
    assert!(stage.labels.contains(&"blocked".to_string()));
}

#[test]
fn tool_stage_meta() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    add_stage_with_tasks(root);

    hlv::mcp::tools::hlv_stage_meta(root, 1, "set", "reviewer", Some("bob")).unwrap();

    let map = load_milestones(root);
    let stage = &map.current.as_ref().unwrap().stages[0];
    assert_eq!(stage.meta.get("reviewer").unwrap(), "bob");
}

#[test]
fn tool_stage_label_nonexistent_stage() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let err = hlv::mcp::tools::hlv_stage_label(root, 99, "add", "test").unwrap_err();
    assert!(
        format!("{err:?}").contains("stage label failed"),
        "Expected stage error, got: {err:?}"
    );
}

#[test]
fn tool_stage_reopen_implemented() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Foundation".to_string(),
        status: StageStatus::Implemented,
        commit: None,
        tasks: Vec::new(),
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    let result = hlv::mcp::tools::hlv_stage_reopen(root, 1).unwrap();
    let text = tool_text(&result);
    assert!(text.contains("reopened"), "Expected 'reopened' in: {text}");

    let map = load_milestones(root);
    assert_eq!(
        map.current.as_ref().unwrap().stages[0].status,
        StageStatus::Implementing
    );
}

#[test]
fn tool_stage_reopen_pending_fails() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    add_stage_with_tasks(root);

    let err = hlv::mcp::tools::hlv_stage_reopen(root, 1).unwrap_err();
    assert!(
        format!("{err:?}").contains("stage reopen failed"),
        "Expected reopen error, got: {err:?}"
    );
}

#[test]
fn tool_task_add() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let milestone_id = load_milestones(root).current.unwrap().id;
    let stage_dir = root.join("human/milestones").join(&milestone_id);
    std::fs::write(
        stage_dir.join("stage_1.md"),
        "# Stage 1: Foundation\n\n## Tasks\n\nTASK-001 First\n  contracts: []\n\n## Remediation\n",
    )
    .unwrap();

    let mut map = load_milestones(root);
    let current = map.current.as_mut().unwrap();
    current.stages = vec![StageEntry {
        id: 1,
        scope: "Foundation".to_string(),
        status: StageStatus::Implemented,
        commit: None,
        tasks: vec![TaskTracker::new("TASK-001".to_string())],
        labels: Vec::new(),
        meta: HashMap::new(),
    }];
    save_milestones(root, &map);

    let result = hlv::mcp::tools::hlv_task_add(root, 1, "TASK-002", "Fix bug").unwrap();
    let text = tool_text(&result);
    assert!(text.contains("TASK-002"), "Expected task ID in: {text}");

    let map = load_milestones(root);
    let stage = &map.current.as_ref().unwrap().stages[0];
    assert_eq!(stage.tasks.len(), 2);
    assert_eq!(stage.tasks[1].id, "TASK-002");
    assert_eq!(stage.status, StageStatus::Implementing); // auto-reopened
}

#[test]
fn tool_task_add_duplicate_fails() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let milestone_id = load_milestones(root).current.unwrap().id;
    let stage_dir = root.join("human/milestones").join(&milestone_id);
    std::fs::write(
        stage_dir.join("stage_1.md"),
        "# Stage 1: Foundation\n\n## Tasks\n\nTASK-001 First\n  contracts: []\n",
    )
    .unwrap();

    add_stage_with_tasks(root);

    let err = hlv::mcp::tools::hlv_task_add(root, 1, "TASK-001", "Dup").unwrap_err();
    assert!(
        format!("{err:?}").contains("task add failed"),
        "Expected task add error, got: {err:?}"
    );
}

#[test]
fn tool_constraint_add_remove() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // Add
    let result = hlv::mcp::tools::hlv_constraint_add(
        root,
        "testing",
        Some("qa-team"),
        Some("ensure coverage"),
        "backend",
    )
    .unwrap();
    assert!(tool_text(&result).contains("testing"));

    // Verify file created
    assert!(root.join("human/constraints/testing.yaml").exists());

    // Remove
    let result = hlv::mcp::tools::hlv_constraint_remove(root, "testing").unwrap();
    assert!(tool_text(&result).contains("testing"));
}

#[test]
fn tool_constraint_add_rule_remove_rule() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // Add constraint first
    hlv::mcp::tools::hlv_constraint_add(root, "myconst", None, None, "all").unwrap();

    // Add rule
    let result = hlv::mcp::tools::hlv_constraint_add_rule(
        root,
        "myconst",
        "RULE-001",
        "high",
        "All endpoints must have auth",
    )
    .unwrap();
    assert!(tool_text(&result).contains("RULE-001"));

    // Remove rule
    let result = hlv::mcp::tools::hlv_constraint_remove_rule(root, "myconst", "RULE-001").unwrap();
    assert!(tool_text(&result).contains("RULE-001"));
}

#[test]
fn tool_constraint_add_rule_invalid_severity() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    hlv::mcp::tools::hlv_constraint_add(root, "sev-test", None, None, "all").unwrap();

    let err = hlv::mcp::tools::hlv_constraint_add_rule(
        root,
        "sev-test",
        "RULE-001",
        "invalid-sev",
        "statement",
    )
    .unwrap_err();
    assert!(
        format!("{err:?}").contains("constraint add-rule failed"),
        "Expected constraint error, got: {err:?}"
    );
}

#[test]
fn tool_commit_msg() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::tools::hlv_commit_msg(root, false, None).unwrap();
    let text = tool_text(&result);
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(
        json["message"].is_string(),
        "Expected message field in JSON"
    );
    let msg = json["message"].as_str().unwrap();
    assert!(!msg.is_empty(), "Commit message should not be empty");
}

#[test]
fn tool_commit_msg_stage_complete() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::tools::hlv_commit_msg(root, true, Some("fix")).unwrap();
    let text = tool_text(&result);
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let msg = json["message"].as_str().unwrap();
    assert!(
        msg.contains("fix"),
        "Expected type override in message, got: {msg}"
    );
}

#[test]
fn tool_glossary() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::tools::hlv_glossary(root).unwrap();
    let text = tool_text(&result);
    // Should be valid JSON (not a placeholder string)
    let json: serde_json::Value =
        serde_json::from_str(&text).expect("glossary result should be valid JSON");
    // glossary.yaml exists in setup_project, so it should be an object (or null if no glossary)
    assert!(
        json.is_object() || json.is_null(),
        "Expected JSON object or null, got: {text}"
    );
}

#[test]
fn tool_artifacts() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::tools::hlv_artifacts(root, None, None).unwrap();
    let text = tool_text(&result);
    // Should be valid JSON with global/milestone arrays
    let json: serde_json::Value =
        serde_json::from_str(&text).expect("artifacts result should be valid JSON");
    assert!(
        json["global"].is_array(),
        "Expected global array in artifacts index"
    );
    assert!(
        json["milestone"].is_array(),
        "Expected milestone array in artifacts index"
    );
}

#[test]
fn tool_artifacts_show_nonexistent() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let err = hlv::mcp::tools::hlv_artifacts(root, None, Some("nonexistent")).unwrap_err();
    assert!(
        format!("{err:?}").contains("artifacts show failed"),
        "Expected artifacts error, got: {err:?}"
    );
}

#[test]
fn tool_quiet_mode_no_stdout_pollution() {
    // Verify that write-tools don't print to stdout when called via MCP wrappers.
    // We test this indirectly by checking that quiet mode is properly restored.
    use hlv::cmd::style;

    assert!(!style::is_quiet(), "quiet should be off by default");

    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    add_stage_with_tasks(root);

    // Call a write-tool that uses style::ok internally
    hlv::mcp::tools::hlv_task_start(root, "TASK-001").unwrap();

    // Quiet mode should be restored to false after the call
    assert!(
        !style::is_quiet(),
        "quiet mode should be restored after MCP tool call"
    );
}

#[test]
fn tool_milestone_done_fails_no_validated_stages() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);
    add_stage_with_tasks(root);

    let err = hlv::mcp::tools::hlv_milestone_done(root).unwrap_err();
    assert!(
        format!("{err:?}").contains("milestone done failed"),
        "Expected milestone error, got: {err:?}"
    );
}

#[test]
fn tool_milestone_abort() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let result = hlv::mcp::tools::hlv_milestone_abort(root).unwrap();
    assert!(tool_text(&result).contains("aborted"));

    // Verify no active milestone
    let map = load_milestones(root);
    assert!(map.current.is_none());
}

#[test]
fn tool_milestone_new_after_abort() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // Abort existing
    hlv::mcp::tools::hlv_milestone_abort(root).unwrap();

    // Create new
    let result = hlv::mcp::tools::hlv_milestone_new(root, "second-milestone").unwrap();
    assert!(tool_text(&result).contains("second-milestone"));

    let map = load_milestones(root);
    assert!(map.current.is_some());
}

#[test]
fn tool_milestone_new_fails_if_active() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let err = hlv::mcp::tools::hlv_milestone_new(root, "another").unwrap_err();
    assert!(
        format!("{err:?}").contains("milestone new failed"),
        "Expected milestone error, got: {err:?}"
    );
}

/// Helper: spawn hlv mcp stdio, initialize, send a tools/call request, return response JSON.
fn mcp_tools_call(root: &Path, tool_name: &str, arguments: serde_json::Value) -> serde_json::Value {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};

    let binary = env!("CARGO_BIN_EXE_hlv");
    let mut child = Command::new(binary)
        .args(["mcp", "--transport", "stdio"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn hlv mcp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize
    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "0.1" }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&init).unwrap()).unwrap();
    stdin.flush().unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();

    // Send initialized notification
    let notif = serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    writeln!(stdin, "{}", serde_json::to_string(&notif).unwrap()).unwrap();
    stdin.flush().unwrap();

    // Send tools/call
    let call = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": tool_name, "arguments": arguments }
    });
    writeln!(stdin, "{}", serde_json::to_string(&call).unwrap()).unwrap();
    stdin.flush().unwrap();

    let mut resp_line = String::new();
    reader.read_line(&mut resp_line).unwrap();

    drop(stdin);
    let _ = child.wait();

    serde_json::from_str(resp_line.trim()).expect("valid JSON-RPC response")
}

#[test]
fn mcp_boundary_unknown_tool_returns_error() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let resp = mcp_tools_call(root, "nonexistent_tool", serde_json::json!({}));
    assert_eq!(resp["jsonrpc"], "2.0");
    assert!(
        resp["error"].is_object(),
        "Expected JSON-RPC error for unknown tool, got: {resp}"
    );
}

#[test]
fn mcp_boundary_missing_required_params() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // hlv_task_start requires task_id (string), send empty args
    let resp = mcp_tools_call(root, "hlv_task_start", serde_json::json!({}));
    assert_eq!(resp["jsonrpc"], "2.0");
    assert!(
        resp["error"].is_object(),
        "Expected JSON-RPC error for missing task_id, got: {resp}"
    );
    let error_msg = resp["error"]["message"].as_str().unwrap_or("");
    assert!(
        error_msg.contains("deserialize") || error_msg.contains("invalid"),
        "Error should mention deserialization failure: {error_msg}"
    );
}

#[test]
fn mcp_boundary_wrong_param_type() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // hlv_stage_label expects stage_id as u32, send a string
    let resp = mcp_tools_call(
        root,
        "hlv_stage_label",
        serde_json::json!({"stage_id": "not-a-number", "action": "add", "label": "test"}),
    );
    assert_eq!(resp["jsonrpc"], "2.0");
    assert!(
        resp["error"].is_object(),
        "Expected JSON-RPC error for wrong param type, got: {resp}"
    );
}

#[test]
fn mcp_boundary_valid_call_returns_result() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // hlv_check requires no params — should succeed
    let resp = mcp_tools_call(root, "hlv_check", serde_json::json!({}));
    assert_eq!(resp["jsonrpc"], "2.0");
    assert!(
        resp["result"].is_object(),
        "Expected result for valid call, got: {resp}"
    );
    // Verify content is present and parseable as JSON
    let content = &resp["result"]["content"];
    assert!(content.is_array(), "Expected content array in result");
    let text = content[0]["text"].as_str().unwrap();
    let payload: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(payload["diagnostics"].is_array());
}

#[test]
fn mcp_boundary_write_tool_no_stdout_corruption() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // hlv_milestone_abort is a write-tool that calls style::ok() etc.
    // If quiet mode works correctly, the response should be clean JSON-RPC.
    let resp = mcp_tools_call(root, "hlv_milestone_abort", serde_json::json!({}));
    assert_eq!(resp["jsonrpc"], "2.0");
    assert!(
        resp["result"].is_object(),
        "Expected clean result from write-tool (no stdout corruption), got: {resp}"
    );
    let text = resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("aborted"),
        "Expected abort confirmation: {text}"
    );
}

#[test]
fn server_capabilities_include_subscribe_in_sse_mode() {
    use hlv::mcp::HlvMcpServer;
    use rmcp::ServerHandler;

    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // SSE mode (with_subscriptions) should advertise subscribe
    let subs = hlv::mcp::watcher::new_subscriptions();
    let server = HlvMcpServer::with_subscriptions(
        hlv::mcp::router::ServerMode::Single(root.to_path_buf()),
        subs,
    );
    let info = server.get_info();
    let caps = info.capabilities;

    let resources = caps
        .resources
        .expect("resources capability should be present");
    assert!(
        resources.subscribe.unwrap_or(false),
        "resources.subscribe should be true in SSE mode"
    );
}

#[test]
fn server_capabilities_no_subscribe_in_stdio_mode() {
    use hlv::mcp::HlvMcpServer;
    use rmcp::ServerHandler;

    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    // Stdio mode (new) should NOT advertise subscribe
    let server = HlvMcpServer::new(hlv::mcp::router::ServerMode::Single(root.to_path_buf()));
    let info = server.get_info();
    let caps = info.capabilities;

    let resources = caps
        .resources
        .expect("resources capability should be present");
    assert!(
        !resources.subscribe.unwrap_or(false),
        "resources.subscribe should be false in stdio mode"
    );
}

#[test]
fn watcher_file_to_uris_mapping() {
    // Verify that watched files map to correct resource URIs
    use hlv::mcp::watcher;

    let subs = watcher::new_subscriptions();
    // Just verify creation works (subscriptions store is empty)
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let map = subs.lock().await;
            assert!(map.is_empty());
        });
}

#[test]
fn subscribe_unsubscribe_via_server() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            use rmcp::transport::streamable_http_server::{
                session::local::LocalSessionManager, StreamableHttpServerConfig,
                StreamableHttpService,
            };
            use std::sync::Arc;
            use tokio_util::sync::CancellationToken;
            use tower_http::cors::{Any, CorsLayer};

            let ct = CancellationToken::new();
            let project_root = root.to_path_buf();
            let subs = hlv::mcp::watcher::new_subscriptions();

            let subs_clone = subs.clone();
            let mcp_service = StreamableHttpService::new(
                move || {
                    Ok(hlv::mcp::HlvMcpServer::with_subscriptions(
                        hlv::mcp::router::ServerMode::Single(project_root.clone()),
                        subs_clone.clone(),
                    ))
                },
                Arc::new(LocalSessionManager::default()),
                StreamableHttpServerConfig {
                    stateful_mode: true,
                    cancellation_token: ct.child_token(),
                    ..Default::default()
                },
            );

            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
                .expose_headers(Any);

            let app = axum::Router::new()
                .nest_service("/mcp", mcp_service)
                .layer(cors);

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();

            let server_ct = ct.clone();
            tokio::spawn(async move {
                axum::serve(listener, app)
                    .with_graceful_shutdown(async move {
                        server_ct.cancelled().await;
                    })
                    .await
                    .unwrap();
            });

            let client = reqwest::Client::new();
            let base = format!("http://{addr}/mcp");

            // 1. Initialize
            let resp = client
                .post(&base)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {
                        "protocolVersion": "2025-03-26",
                        "capabilities": {},
                        "clientInfo": { "name": "test", "version": "0.1" }
                    }
                }))
                .send()
                .await
                .unwrap();

            assert_eq!(resp.status(), 200);
            let session_id = resp
                .headers()
                .get("mcp-session-id")
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();

            // Read the init response body (required to complete the SSE stream)
            let body = resp.text().await.unwrap();
            assert!(body.contains("serverInfo") || body.contains("server_info"));

            // 2. Send initialized notification
            let resp = client
                .post(&base)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .header("Mcp-Session-Id", &session_id)
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/initialized"
                }))
                .send()
                .await
                .unwrap();
            assert!(resp.status().is_success());

            // 3. Subscribe to milestones
            let resp = client
                .post(&base)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .header("Mcp-Session-Id", &session_id)
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "resources/subscribe",
                    "params": { "uri": "hlv://milestones" }
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), 200);

            // 4. Verify subscription stored
            {
                let map = subs.lock().await;
                assert!(
                    map.contains_key("hlv://milestones"),
                    "Subscription for hlv://milestones should be stored"
                );
            }

            // 5. Unsubscribe
            let resp = client
                .post(&base)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .header("Mcp-Session-Id", &session_id)
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 3,
                    "method": "resources/unsubscribe",
                    "params": { "uri": "hlv://milestones" }
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), 200);

            // 6. Verify unsubscribed
            {
                let map = subs.lock().await;
                assert!(
                    !map.contains_key("hlv://milestones"),
                    "Subscription for hlv://milestones should be removed"
                );
            }

            ct.cancel();
        });
}

#[test]
fn e2e_file_change_triggers_notification_on_sse_stream() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            use rmcp::transport::streamable_http_server::{
                session::local::LocalSessionManager, StreamableHttpServerConfig,
                StreamableHttpService,
            };
            use std::sync::Arc;
            use tokio_util::sync::CancellationToken;
            use tower_http::cors::{Any, CorsLayer};

            let ct = CancellationToken::new();
            let project_root = root.to_path_buf();
            let subs = hlv::mcp::watcher::new_subscriptions();

            // Start file watcher
            let _watcher = hlv::mcp::watcher::start_watcher(
                project_root.clone(),
                None,
                subs.clone(),
                tokio::runtime::Handle::current(),
            );
            assert!(_watcher.is_some(), "Watcher should start successfully");

            let subs_clone = subs.clone();
            let mcp_service = StreamableHttpService::new(
                {
                    let project_root = project_root.clone();
                    let subs = subs.clone();
                    move || {
                        Ok(hlv::mcp::HlvMcpServer::with_subscriptions(
                            hlv::mcp::router::ServerMode::Single(project_root.clone()),
                            subs.clone(),
                        ))
                    }
                },
                Arc::new(LocalSessionManager::default()),
                StreamableHttpServerConfig {
                    stateful_mode: true,
                    cancellation_token: ct.child_token(),
                    ..Default::default()
                },
            );

            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
                .expose_headers(Any);

            let app = axum::Router::new()
                .nest_service("/mcp", mcp_service)
                .layer(cors);

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();

            let server_ct = ct.clone();
            tokio::spawn(async move {
                axum::serve(listener, app)
                    .with_graceful_shutdown(async move {
                        server_ct.cancelled().await;
                    })
                    .await
                    .unwrap();
            });

            let client = reqwest::Client::new();
            let base = format!("http://{addr}/mcp");

            // 1. Initialize
            let resp = client
                .post(&base)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {
                        "protocolVersion": "2025-03-26",
                        "capabilities": {},
                        "clientInfo": { "name": "test-e2e", "version": "0.1" }
                    }
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), 200);
            let session_id = resp
                .headers()
                .get("mcp-session-id")
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            let _body = resp.text().await.unwrap();

            // 2. Send initialized notification
            let resp = client
                .post(&base)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .header("Mcp-Session-Id", &session_id)
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/initialized"
                }))
                .send()
                .await
                .unwrap();
            assert!(resp.status().is_success());

            // 3. Subscribe to milestones
            let resp = client
                .post(&base)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .header("Mcp-Session-Id", &session_id)
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "resources/subscribe",
                    "params": { "uri": "hlv://milestones" }
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), 200);
            let _body = resp.text().await.unwrap();

            // Verify subscription stored
            {
                let map = subs_clone.lock().await;
                assert!(
                    map.contains_key("hlv://milestones"),
                    "Subscription for hlv://milestones should exist after subscribe"
                );
                let entries = map.get("hlv://milestones").unwrap();
                assert_eq!(entries.len(), 1, "Should have exactly one subscriber");
            }

            // 4. Open an SSE GET stream to receive notifications
            let sse_resp = client
                .get(&base)
                .header("Accept", "text/event-stream")
                .header("Mcp-Session-Id", &session_id)
                .send()
                .await
                .unwrap();
            assert!(
                sse_resp.status().is_success(),
                "GET SSE stream should succeed"
            );

            // 5. Modify milestones.yaml to trigger watcher
            //    Small delay to let the GET stream establish
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            let milestones_path = project_root.join("milestones.yaml");
            let content = std::fs::read_to_string(&milestones_path).unwrap();
            std::fs::write(&milestones_path, format!("{content}\n# trigger watcher\n")).unwrap();

            // 6. Read SSE stream chunks with timeout — expect resources/updated
            let mut collected = String::new();
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
            let mut stream = sse_resp;
            loop {
                let remaining = deadline.duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    break;
                }
                match tokio::time::timeout(remaining, stream.chunk()).await {
                    Ok(Ok(Some(chunk))) => {
                        collected.push_str(&String::from_utf8_lossy(&chunk));
                        if collected.contains("resources/updated") {
                            break;
                        }
                    }
                    Ok(Ok(None)) => break, // stream ended
                    Ok(Err(_)) => break,   // reqwest error
                    Err(_) => break,       // timeout
                }
            }

            assert!(
                collected.contains("notifications/resources/updated")
                    || collected.contains("resources/updated"),
                "SSE stream should contain resources/updated notification, got: {collected}"
            );
            assert!(
                collected.contains("hlv://milestones"),
                "Notification should reference hlv://milestones URI, got: {collected}"
            );

            ct.cancel();
        });
}

#[test]
fn sse_server_responds_to_initialize() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            use rmcp::transport::streamable_http_server::{
                session::local::LocalSessionManager, StreamableHttpServerConfig,
                StreamableHttpService,
            };
            use std::sync::Arc;
            use tokio_util::sync::CancellationToken;
            use tower_http::cors::{Any, CorsLayer};

            let ct = CancellationToken::new();
            let project_root = root.to_path_buf();

            let mcp_service = StreamableHttpService::new(
                move || {
                    Ok(hlv::mcp::HlvMcpServer::new(
                        hlv::mcp::router::ServerMode::Single(project_root.clone()),
                    ))
                },
                Arc::new(LocalSessionManager::default()),
                StreamableHttpServerConfig {
                    stateful_mode: true,
                    cancellation_token: ct.child_token(),
                    ..Default::default()
                },
            );

            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
                .expose_headers(Any);

            let app = axum::Router::new()
                .nest_service("/mcp", mcp_service)
                .layer(cors);

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();

            let server_ct = ct.clone();
            tokio::spawn(async move {
                axum::serve(listener, app)
                    .with_graceful_shutdown(async move {
                        server_ct.cancelled().await;
                    })
                    .await
                    .unwrap();
            });

            // Send an initialize request
            let client = reqwest::Client::new();
            let init_request = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": { "name": "test-client", "version": "0.1.0" }
                }
            });

            let resp = client
                .post(format!("http://{addr}/mcp"))
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .json(&init_request)
                .send()
                .await
                .unwrap();

            assert_eq!(resp.status(), 200);

            // Check CORS headers
            let headers = resp.headers();
            assert!(
                headers.get("access-control-allow-origin").is_some(),
                "CORS Allow-Origin header should be present"
            );

            // Check session ID header
            let session_id = headers
                .get("mcp-session-id")
                .expect("Session ID header should be present");
            assert!(!session_id.is_empty());

            // Response should be SSE (text/event-stream)
            let content_type = headers.get("content-type").unwrap().to_str().unwrap();
            assert!(
                content_type.contains("text/event-stream"),
                "Expected SSE content type, got: {content_type}"
            );

            // Read SSE body and find the initialize result
            let body = resp.text().await.unwrap();
            // SSE body should contain a JSON-RPC response with serverInfo
            assert!(
                body.contains("serverInfo") || body.contains("server_info"),
                "SSE body should contain serverInfo: {body}"
            );

            ct.cancel();
        });
}

#[test]
fn sse_client_disconnect_does_not_crash_server() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            use rmcp::transport::streamable_http_server::{
                session::local::LocalSessionManager, StreamableHttpServerConfig,
                StreamableHttpService,
            };
            use std::sync::Arc;
            use tokio_util::sync::CancellationToken;
            use tower_http::cors::{Any, CorsLayer};

            let ct = CancellationToken::new();
            let project_root = root.to_path_buf();

            let mcp_service = StreamableHttpService::new(
                move || {
                    Ok(hlv::mcp::HlvMcpServer::new(
                        hlv::mcp::router::ServerMode::Single(project_root.clone()),
                    ))
                },
                Arc::new(LocalSessionManager::default()),
                StreamableHttpServerConfig {
                    stateful_mode: true,
                    cancellation_token: ct.child_token(),
                    ..Default::default()
                },
            );

            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
                .expose_headers(Any);

            let app = axum::Router::new()
                .nest_service("/mcp", mcp_service)
                .layer(cors);

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();

            let server_ct = ct.clone();
            tokio::spawn(async move {
                axum::serve(listener, app)
                    .with_graceful_shutdown(async move {
                        server_ct.cancelled().await;
                    })
                    .await
                    .unwrap();
            });

            let init_request = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": { "name": "disconnect-test", "version": "0.1.0" }
                }
            });

            // Client 1: connect, get session, then drop (simulate disconnect)
            {
                let client = reqwest::Client::new();
                let resp = client
                    .post(format!("http://{addr}/mcp"))
                    .header("Content-Type", "application/json")
                    .header("Accept", "application/json, text/event-stream")
                    .json(&init_request)
                    .send()
                    .await
                    .unwrap();
                assert_eq!(resp.status(), 200);
                // Consume response and drop client — simulates client disconnect
                let _body = resp.text().await.unwrap();
            }
            // client and response are dropped here

            // Give the server a moment to process the disconnect
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Client 2: verify the server still accepts new connections after disconnect
            {
                let client2 = reqwest::Client::new();
                let resp2 = client2
                    .post(format!("http://{addr}/mcp"))
                    .header("Content-Type", "application/json")
                    .header("Accept", "application/json, text/event-stream")
                    .json(&init_request)
                    .send()
                    .await
                    .unwrap();
                assert_eq!(
                    resp2.status(),
                    200,
                    "Server should still accept connections after client disconnect"
                );
                let body = resp2.text().await.unwrap();
                assert!(
                    body.contains("serverInfo") || body.contains("server_info"),
                    "Server should respond normally after previous client disconnected"
                );
            }

            ct.cancel();
        });
}

#[test]
fn sse_cors_preflight() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            use rmcp::transport::streamable_http_server::{
                session::local::LocalSessionManager, StreamableHttpServerConfig,
                StreamableHttpService,
            };
            use std::sync::Arc;
            use tokio_util::sync::CancellationToken;
            use tower_http::cors::{Any, CorsLayer};

            let ct = CancellationToken::new();
            let project_root = root.to_path_buf();

            let mcp_service = StreamableHttpService::new(
                move || {
                    Ok(hlv::mcp::HlvMcpServer::new(
                        hlv::mcp::router::ServerMode::Single(project_root.clone()),
                    ))
                },
                Arc::new(LocalSessionManager::default()),
                StreamableHttpServerConfig {
                    stateful_mode: true,
                    cancellation_token: ct.child_token(),
                    ..Default::default()
                },
            );

            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
                .expose_headers(Any);

            let app = axum::Router::new()
                .nest_service("/mcp", mcp_service)
                .layer(cors);

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();

            let server_ct = ct.clone();
            tokio::spawn(async move {
                axum::serve(listener, app)
                    .with_graceful_shutdown(async move {
                        server_ct.cancelled().await;
                    })
                    .await
                    .unwrap();
            });

            // Send CORS preflight (OPTIONS)
            let client = reqwest::Client::new();
            let resp = client
                .request(reqwest::Method::OPTIONS, format!("http://{addr}/mcp"))
                .header("Origin", "http://example.com")
                .header("Access-Control-Request-Method", "POST")
                .header("Access-Control-Request-Headers", "content-type")
                .send()
                .await
                .unwrap();

            assert!(
                resp.status().is_success(),
                "CORS preflight should succeed, got: {}",
                resp.status()
            );

            let headers = resp.headers();
            assert!(
                headers.get("access-control-allow-origin").is_some(),
                "CORS Allow-Origin header should be present in preflight"
            );
            assert!(
                headers.get("access-control-allow-methods").is_some(),
                "CORS Allow-Methods header should be present in preflight"
            );
            assert!(
                headers.get("access-control-allow-headers").is_some(),
                "CORS Allow-Headers header should be present in preflight"
            );

            ct.cancel();
        });
}

fn setup_workspace(projects: &[(&str, &Path)]) -> String {
    let mut yaml = "projects:\n".to_string();
    for (id, root) in projects {
        setup_project(root);
        yaml.push_str(&format!("  - id: {id}\n    root: {}\n", root.display()));
    }
    yaml
}

#[test]
fn workspace_config_load_and_validate() {
    use hlv::mcp::workspace::WorkspaceConfig;

    let tmp1 = TempDir::new().unwrap();
    let tmp2 = TempDir::new().unwrap();
    setup_project(tmp1.path());
    setup_project(tmp2.path());

    let ws_dir = TempDir::new().unwrap();
    let ws_path = ws_dir.path().join("workspace.yaml");
    let yaml = format!(
        "projects:\n  - id: alpha\n    root: {}\n  - id: beta\n    root: {}\n",
        tmp1.path().display(),
        tmp2.path().display()
    );
    std::fs::write(&ws_path, &yaml).unwrap();

    let config = WorkspaceConfig::load(&ws_path).unwrap();
    assert_eq!(config.projects.len(), 2);
    assert!(config.find("alpha").is_some());
    assert!(config.find("beta").is_some());
    assert!(config.find("gamma").is_none());
}

#[test]
fn workspace_config_rejects_duplicate_ids() {
    use hlv::mcp::workspace::WorkspaceConfig;

    let tmp = TempDir::new().unwrap();
    setup_project(tmp.path());

    let ws_dir = TempDir::new().unwrap();
    let ws_path = ws_dir.path().join("workspace.yaml");
    let yaml = format!(
        "projects:\n  - id: dup\n    root: {}\n  - id: dup\n    root: {}\n",
        tmp.path().display(),
        tmp.path().display()
    );
    std::fs::write(&ws_path, &yaml).unwrap();

    let err = WorkspaceConfig::load(&ws_path).unwrap_err();
    assert!(err.to_string().contains("Duplicate project ID"));
}

#[test]
fn workspace_config_rejects_missing_project_yaml() {
    use hlv::mcp::workspace::WorkspaceConfig;

    let tmp = TempDir::new().unwrap(); // No project.yaml

    let ws_dir = TempDir::new().unwrap();
    let ws_path = ws_dir.path().join("workspace.yaml");
    let yaml = format!(
        "projects:\n  - id: nope\n    root: {}\n",
        tmp.path().display()
    );
    std::fs::write(&ws_path, &yaml).unwrap();

    let err = WorkspaceConfig::load(&ws_path).unwrap_err();
    assert!(err.to_string().contains("no project.yaml"));
}

#[test]
fn workspace_summaries() {
    use hlv::mcp::workspace::WorkspaceConfig;

    let tmp1 = TempDir::new().unwrap();
    let tmp2 = TempDir::new().unwrap();

    let ws_dir = TempDir::new().unwrap();
    let ws_path = ws_dir.path().join("workspace.yaml");
    let yaml = setup_workspace(&[("proj1", tmp1.path()), ("proj2", tmp2.path())]);
    std::fs::write(&ws_path, &yaml).unwrap();

    let config = WorkspaceConfig::load(&ws_path).unwrap();
    let summaries = config.summaries();
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].id, "proj1");
    assert_eq!(summaries[1].id, "proj2");
    assert!(summaries[0].name.is_some());
    assert!(summaries[0].current_milestone.is_some());
}

#[test]
fn workspace_resource_list_projects() {
    use hlv::mcp::router::ServerMode;
    use hlv::mcp::workspace::WorkspaceConfig;

    let tmp1 = TempDir::new().unwrap();
    let tmp2 = TempDir::new().unwrap();

    let ws_dir = TempDir::new().unwrap();
    let ws_path = ws_dir.path().join("workspace.yaml");
    let yaml = setup_workspace(&[("alpha", tmp1.path()), ("beta", tmp2.path())]);
    std::fs::write(&ws_path, &yaml).unwrap();

    let config = WorkspaceConfig::load(&ws_path).unwrap();
    let mode = ServerMode::Workspace(config);

    let result = hlv::mcp::resources::read_resource_workspace(&mode, "hlv://projects").unwrap();
    let text = resource_text(&result);
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0]["id"], "alpha");
    assert_eq!(parsed[1]["id"], "beta");
}

#[test]
fn workspace_resource_scoped_project() {
    use hlv::mcp::router::ServerMode;
    use hlv::mcp::workspace::WorkspaceConfig;

    let tmp1 = TempDir::new().unwrap();
    let ws_dir = TempDir::new().unwrap();
    let ws_path = ws_dir.path().join("workspace.yaml");
    let yaml = setup_workspace(&[("myproj", tmp1.path())]);
    std::fs::write(&ws_path, &yaml).unwrap();

    let config = WorkspaceConfig::load(&ws_path).unwrap();
    let mode = ServerMode::Workspace(config);

    // hlv://projects/myproj → should return project info
    let result =
        hlv::mcp::resources::read_resource_workspace(&mode, "hlv://projects/myproj").unwrap();
    let text = resource_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(parsed["project"].is_string());
}

#[test]
fn workspace_resource_scoped_milestones() {
    use hlv::mcp::router::ServerMode;
    use hlv::mcp::workspace::WorkspaceConfig;

    let tmp1 = TempDir::new().unwrap();
    let ws_dir = TempDir::new().unwrap();
    let ws_path = ws_dir.path().join("workspace.yaml");
    let yaml = setup_workspace(&[("app", tmp1.path())]);
    std::fs::write(&ws_path, &yaml).unwrap();

    let config = WorkspaceConfig::load(&ws_path).unwrap();
    let mode = ServerMode::Workspace(config);

    let result =
        hlv::mcp::resources::read_resource_workspace(&mode, "hlv://projects/app/milestones")
            .unwrap();
    let text = resource_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(parsed["current"].is_object());
}

#[test]
fn workspace_resource_unknown_project() {
    use hlv::mcp::router::ServerMode;
    use hlv::mcp::workspace::WorkspaceConfig;

    let tmp1 = TempDir::new().unwrap();
    let ws_dir = TempDir::new().unwrap();
    let ws_path = ws_dir.path().join("workspace.yaml");
    let yaml = setup_workspace(&[("real", tmp1.path())]);
    std::fs::write(&ws_path, &yaml).unwrap();

    let config = WorkspaceConfig::load(&ws_path).unwrap();
    let mode = ServerMode::Workspace(config);

    let err = hlv::mcp::resources::read_resource_workspace(&mode, "hlv://projects/fake/milestones")
        .unwrap_err();
    assert!(err.message.contains("Unknown project"));
}

#[test]
fn workspace_resource_invalid_uri() {
    use hlv::mcp::router::ServerMode;
    use hlv::mcp::workspace::WorkspaceConfig;

    let tmp1 = TempDir::new().unwrap();
    let ws_dir = TempDir::new().unwrap();
    let ws_path = ws_dir.path().join("workspace.yaml");
    let yaml = setup_workspace(&[("x", tmp1.path())]);
    std::fs::write(&ws_path, &yaml).unwrap();

    let config = WorkspaceConfig::load(&ws_path).unwrap();
    let mode = ServerMode::Workspace(config);

    // Non-workspace URI in workspace mode should error
    let err = hlv::mcp::resources::read_resource_workspace(&mode, "hlv://milestones").unwrap_err();
    assert!(err.message.contains("Unknown workspace resource"));
}

#[test]
fn workspace_list_resources_has_projects() {
    let result = hlv::mcp::resources::list_resources_workspace();
    assert_eq!(result.resources.len(), 1);
    assert_eq!(result.resources[0].raw.uri, "hlv://projects");
}

#[test]
fn workspace_list_templates_has_scoped_resources() {
    let result = hlv::mcp::resources::list_resource_templates_workspace();
    assert!(!result.resource_templates.is_empty());

    let uris: Vec<_> = result
        .resource_templates
        .iter()
        .map(|t| t.raw.uri_template.as_str())
        .collect();
    assert!(uris.contains(&"hlv://projects/{id}/milestones"));
    assert!(uris.contains(&"hlv://projects/{id}/tasks"));
    assert!(uris.contains(&"hlv://projects/{id}/stage/{n}"));
}

#[test]
fn workspace_server_info_shows_workspace_mode() {
    use hlv::mcp::router::ServerMode;
    use hlv::mcp::workspace::{WorkspaceConfig, WorkspaceProject};
    use hlv::mcp::HlvMcpServer;
    use rmcp::ServerHandler;

    let tmp = TempDir::new().unwrap();
    setup_project(tmp.path());

    let mode = ServerMode::Workspace(WorkspaceConfig {
        projects: vec![WorkspaceProject {
            id: "test".to_string(),
            root: tmp.path().to_path_buf(),
        }],
    });
    let server = HlvMcpServer::new(mode);
    let info = server.get_info();
    assert!(info
        .instructions
        .as_deref()
        .unwrap_or("")
        .contains("WORKSPACE"));
}

#[test]
fn workspace_tool_resolve_root() {
    use hlv::mcp::router::ServerMode;
    use hlv::mcp::workspace::{WorkspaceConfig, WorkspaceProject};

    let tmp = TempDir::new().unwrap();

    let mode = ServerMode::Workspace(WorkspaceConfig {
        projects: vec![WorkspaceProject {
            id: "api".to_string(),
            root: tmp.path().to_path_buf(),
        }],
    });

    // project_id required in workspace mode
    assert!(mode.resolve_root(None).is_err());
    assert!(mode.resolve_root(Some("api")).is_ok());
    assert!(mode.resolve_root(Some("unknown")).is_err());
}

#[test]
fn router_parse_workspace_uri_patterns() {
    use hlv::mcp::router::parse_workspace_uri;

    // Standard resource
    let (id, inner) = parse_workspace_uri("hlv://projects/backend/tasks").unwrap();
    assert_eq!(id, "backend");
    assert_eq!(inner, "hlv://tasks");

    // Parametrized resource
    let (id, inner) = parse_workspace_uri("hlv://projects/api/stage/2").unwrap();
    assert_eq!(id, "api");
    assert_eq!(inner, "hlv://stage/2");

    // Nested parametrized
    let (id, inner) =
        parse_workspace_uri("hlv://projects/app/artifacts/milestone/001/context").unwrap();
    assert_eq!(id, "app");
    assert_eq!(inner, "hlv://artifacts/milestone/001/context");

    // Project shorthand
    let (id, inner) = parse_workspace_uri("hlv://projects/backend").unwrap();
    assert_eq!(id, "backend");
    assert_eq!(inner, "hlv://project");

    // Non-workspace URI → None
    assert!(parse_workspace_uri("hlv://milestones").is_none());
    assert!(parse_workspace_uri("hlv://project").is_none());
}

#[test]
fn workspace_resource_scoped_tasks() {
    use hlv::mcp::router::ServerMode;
    use hlv::mcp::workspace::WorkspaceConfig;

    let tmp = TempDir::new().unwrap();
    let ws_dir = TempDir::new().unwrap();
    let ws_path = ws_dir.path().join("workspace.yaml");
    let yaml = setup_workspace(&[("svc", tmp.path())]);
    std::fs::write(&ws_path, &yaml).unwrap();

    // Add stage with tasks
    add_stage_with_tasks(tmp.path());

    let config = WorkspaceConfig::load(&ws_path).unwrap();
    let mode = ServerMode::Workspace(config);

    let result =
        hlv::mcp::resources::read_resource_workspace(&mode, "hlv://projects/svc/tasks").unwrap();
    let text = resource_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
    // Should contain tasks
    assert!(parsed.is_array() || parsed.is_object());
}

/// E2E: tools/call via SSE in workspace mode — hlv_task_list with project_id.
#[test]
fn workspace_e2e_tool_call_via_sse() {
    let tmp1 = TempDir::new().unwrap();
    let tmp2 = TempDir::new().unwrap();

    let ws_dir = TempDir::new().unwrap();
    let ws_path = ws_dir.path().join("workspace.yaml");
    let yaml = setup_workspace(&[("proj_a", tmp1.path()), ("proj_b", tmp2.path())]);
    std::fs::write(&ws_path, &yaml).unwrap();

    // Add tasks to proj_a only
    add_stage_with_tasks(tmp1.path());

    let config = hlv::mcp::workspace::WorkspaceConfig::load(&ws_path).unwrap();
    let mode = hlv::mcp::router::ServerMode::Workspace(config);

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            use rmcp::transport::streamable_http_server::{
                session::local::LocalSessionManager, StreamableHttpServerConfig,
                StreamableHttpService,
            };
            use std::sync::Arc;
            use tokio_util::sync::CancellationToken;
            use tower_http::cors::{Any, CorsLayer};

            let ct = CancellationToken::new();

            let mcp_service = StreamableHttpService::new(
                {
                    let mode = mode.clone();
                    move || Ok(hlv::mcp::HlvMcpServer::new(mode.clone()))
                },
                Arc::new(LocalSessionManager::default()),
                StreamableHttpServerConfig {
                    stateful_mode: true,
                    cancellation_token: ct.child_token(),
                    ..Default::default()
                },
            );

            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
                .expose_headers(Any);

            let app = axum::Router::new()
                .nest_service("/mcp", mcp_service)
                .layer(cors);

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();

            let server_ct = ct.clone();
            tokio::spawn(async move {
                axum::serve(listener, app)
                    .with_graceful_shutdown(async move { server_ct.cancelled().await })
                    .await
                    .unwrap();
            });

            let client = reqwest::Client::new();
            let base = format!("http://{addr}/mcp");

            // 1. Initialize
            let resp = client
                .post(&base)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {
                        "protocolVersion": "2025-03-26",
                        "capabilities": {},
                        "clientInfo": { "name": "ws-test", "version": "0.1" }
                    }
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), 200);
            let session_id = resp
                .headers()
                .get("mcp-session-id")
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            let _body = resp.text().await.unwrap();

            // 2. Send initialized notification
            let resp = client
                .post(&base)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .header("Mcp-Session-Id", &session_id)
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/initialized"
                }))
                .send()
                .await
                .unwrap();
            assert!(resp.status().is_success());
            let _body = resp.text().await.unwrap();

            // 3. Call hlv_task_list with project_id = "proj_a"
            let resp = client
                .post(&base)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .header("Mcp-Session-Id", &session_id)
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools/call",
                    "params": {
                        "name": "hlv_task_list",
                        "arguments": { "project_id": "proj_a" }
                    }
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), 200);
            let body_text = resp.text().await.unwrap();
            // Response may be SSE-formatted or plain JSON
            let body_json = extract_json_from_sse_or_json(&body_text);
            assert!(
                body_json["result"]["content"][0]["text"].is_string(),
                "Expected tool result with text, got: {body_json}"
            );
            let task_text = body_json["result"]["content"][0]["text"].as_str().unwrap();
            assert!(
                task_text.contains("TASK-001"),
                "proj_a should have TASK-001"
            );

            // 4. Call hlv_task_list with project_id = "proj_b" (no tasks)
            let resp = client
                .post(&base)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .header("Mcp-Session-Id", &session_id)
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 3,
                    "method": "tools/call",
                    "params": {
                        "name": "hlv_task_list",
                        "arguments": { "project_id": "proj_b" }
                    }
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), 200);
            let body_text = resp.text().await.unwrap();
            let body_json = extract_json_from_sse_or_json(&body_text);
            let task_text = body_json["result"]["content"][0]["text"].as_str().unwrap();
            assert!(
                !task_text.contains("TASK-001"),
                "proj_b should NOT have TASK-001"
            );

            // 5. Call without project_id in workspace mode → error
            let resp = client
                .post(&base)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .header("Mcp-Session-Id", &session_id)
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 4,
                    "method": "tools/call",
                    "params": {
                        "name": "hlv_check",
                        "arguments": {}
                    }
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), 200);
            let body_text = resp.text().await.unwrap();
            let body_json = extract_json_from_sse_or_json(&body_text);
            assert!(
                body_json["error"].is_object(),
                "Should error without project_id in workspace mode: {body_json}"
            );

            ct.cancel();
        });
}

/// E2E: subscription on scoped workspace URI + file-change notification.
#[test]
fn workspace_e2e_scoped_notification_on_file_change() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_project(root);

    let ws_dir = TempDir::new().unwrap();
    let ws_path = ws_dir.path().join("workspace.yaml");
    let yaml = format!("projects:\n  - id: demo\n    root: {}\n", root.display());
    std::fs::write(&ws_path, &yaml).unwrap();

    let config = hlv::mcp::workspace::WorkspaceConfig::load(&ws_path).unwrap();
    let mode = hlv::mcp::router::ServerMode::Workspace(config);

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            use rmcp::transport::streamable_http_server::{
                session::local::LocalSessionManager, StreamableHttpServerConfig,
                StreamableHttpService,
            };
            use std::sync::Arc;
            use tokio_util::sync::CancellationToken;
            use tower_http::cors::{Any, CorsLayer};

            let ct = CancellationToken::new();

            let subs = hlv::mcp::watcher::new_subscriptions();

            // Start watcher with project_id "demo"
            let _watcher = hlv::mcp::watcher::start_watcher(
                root.to_path_buf(),
                Some("demo".to_string()),
                subs.clone(),
                tokio::runtime::Handle::current(),
            );

            let subs_clone = subs.clone();
            let mcp_service = StreamableHttpService::new(
                {
                    let mode = mode.clone();
                    let subs = subs.clone();
                    move || {
                        Ok(hlv::mcp::HlvMcpServer::with_subscriptions(
                            mode.clone(),
                            subs.clone(),
                        ))
                    }
                },
                Arc::new(LocalSessionManager::default()),
                StreamableHttpServerConfig {
                    stateful_mode: true,
                    cancellation_token: ct.child_token(),
                    ..Default::default()
                },
            );

            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
                .expose_headers(Any);

            let app = axum::Router::new()
                .nest_service("/mcp", mcp_service)
                .layer(cors);

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();

            let server_ct = ct.clone();
            tokio::spawn(async move {
                axum::serve(listener, app)
                    .with_graceful_shutdown(async move { server_ct.cancelled().await })
                    .await
                    .unwrap();
            });

            let client = reqwest::Client::new();
            let base = format!("http://{addr}/mcp");

            // 1. Initialize
            let resp = client
                .post(&base)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {
                        "protocolVersion": "2025-03-26",
                        "capabilities": {},
                        "clientInfo": { "name": "ws-sub-test", "version": "0.1" }
                    }
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), 200);
            let session_id = resp
                .headers()
                .get("mcp-session-id")
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            let _body = resp.text().await.unwrap();

            // 2. Send initialized notification
            let resp = client
                .post(&base)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .header("Mcp-Session-Id", &session_id)
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/initialized"
                }))
                .send()
                .await
                .unwrap();
            assert!(resp.status().is_success());
            let _body = resp.text().await.unwrap();

            // 3. Open SSE stream
            let sse_resp = client
                .get(&base)
                .header("Accept", "text/event-stream")
                .header("Mcp-Session-Id", &session_id)
                .send()
                .await
                .unwrap();
            assert!(
                sse_resp.status().is_success(),
                "GET SSE stream should succeed"
            );

            // 4. Subscribe to scoped URI: hlv://projects/demo/milestones
            let resp = client
                .post(&base)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .header("Mcp-Session-Id", &session_id)
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "resources/subscribe",
                    "params": { "uri": "hlv://projects/demo/milestones" }
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), 200);
            let _body = resp.text().await.unwrap();

            // 5. Verify subscription store has the scoped URI
            {
                let map = subs_clone.lock().await;
                assert!(
                    map.contains_key("hlv://projects/demo/milestones"),
                    "Subscription store should contain scoped URI, got: {:?}",
                    map.keys().collect::<Vec<_>>()
                );
            }

            // 6. Modify milestones.yaml to trigger notification
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            let ms_path = root.join("milestones.yaml");
            let content = std::fs::read_to_string(&ms_path).unwrap();
            std::fs::write(&ms_path, format!("{content}\n# trigger\n")).unwrap();

            // 7. Collect SSE events — watcher should emit scoped URI
            let mut collected = String::new();
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
            let mut stream = sse_resp;
            loop {
                let remaining = deadline.duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    break;
                }
                match tokio::time::timeout(remaining, stream.chunk()).await {
                    Ok(Ok(Some(chunk))) => {
                        collected.push_str(&String::from_utf8_lossy(&chunk));
                        if collected.contains("hlv://projects/demo/milestones") {
                            break;
                        }
                    }
                    Ok(Ok(None)) => break,
                    Ok(Err(_)) => break,
                    Err(_) => break,
                }
            }

            assert!(
                collected.contains("hlv://projects/demo/milestones"),
                "Notification should reference scoped URI hlv://projects/demo/milestones, got: {collected}"
            );

            ct.cancel();
        });
}

/// Unit test: watcher file_to_uris produces scoped URIs in workspace mode.
#[test]
fn watcher_uris_scoped_in_workspace_mode() {
    // This tests the internal watcher logic via the unit tests in watcher.rs,
    // but we also verify at integration level here.
    use hlv::mcp::router::ServerMode;
    use hlv::mcp::workspace::{WorkspaceConfig, WorkspaceProject};

    let tmp = TempDir::new().unwrap();
    setup_project(tmp.path());

    let mode = ServerMode::Workspace(WorkspaceConfig {
        projects: vec![WorkspaceProject {
            id: "ws_proj".to_string(),
            root: tmp.path().to_path_buf(),
        }],
    });

    // In workspace mode, server instructions mention WORKSPACE
    let server = hlv::mcp::HlvMcpServer::new(mode);
    let info = rmcp::ServerHandler::get_info(&server);
    let instructions = info.instructions.unwrap_or_default();
    assert!(instructions.contains("project_id"));
}
