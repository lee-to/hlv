use rmcp::{model::*, ErrorData as McpError};
use std::path::Path;

use super::router::{self, ServerMode};
use crate::model::{
    artifact::ArtifactIndex, glossary::Glossary, milestone::MilestoneMap, project::ProjectMap,
};

fn text_resource(uri: &str, name: &str, description: &str) -> Resource {
    RawResource {
        uri: uri.to_string(),
        name: name.to_string(),
        title: None,
        description: Some(description.to_string()),
        mime_type: Some("application/json".to_string()),
        size: None,
        icons: None,
        meta: None,
    }
    .no_annotation()
}

fn tmpl(uri_template: &str, name: &str, description: &str) -> ResourceTemplate {
    RawResourceTemplate {
        uri_template: uri_template.to_string(),
        name: name.to_string(),
        title: None,
        description: Some(description.to_string()),
        mime_type: Some("application/json".to_string()),
        icons: None,
    }
    .no_annotation()
}

pub fn list_resources() -> ListResourcesResult {
    ListResourcesResult {
        resources: vec![
            text_resource(
                "hlv://project",
                "Project",
                "Project configuration and metadata",
            ),
            text_resource(
                "hlv://milestones",
                "Milestones",
                "All milestones (current + history)",
            ),
            text_resource(
                "hlv://contracts",
                "Contracts",
                "List of contracts with metadata",
            ),
            text_resource("hlv://gates", "Gates", "Validation gates policy"),
            text_resource("hlv://constraints", "Constraints", "Project constraints"),
            text_resource("hlv://map", "Map", "LLM-generated code map"),
            text_resource(
                "hlv://workflow",
                "Workflow",
                "Current phase and next actions",
            ),
            text_resource("hlv://tasks", "Tasks", "All tasks across all stages"),
            text_resource("hlv://artifacts", "Artifacts", "Global artifact metadata"),
            text_resource("hlv://plan", "Plan", "Implementation plan"),
            text_resource(
                "hlv://traceability",
                "Traceability",
                "Requirement-contract-code traceability",
            ),
            text_resource("hlv://glossary", "Glossary", "Domain glossary terms"),
        ],
        next_cursor: None,
        meta: None,
    }
}

pub fn list_resource_templates() -> ListResourceTemplatesResult {
    ListResourceTemplatesResult {
        resource_templates: vec![
            tmpl("hlv://stage/{n}", "Stage", "Stage plan by number"),
            tmpl("hlv://contracts/{id}", "Contract", "Single contract by ID"),
            tmpl(
                "hlv://tasks/{n}",
                "Tasks (stage)",
                "Tasks for a specific stage",
            ),
            tmpl(
                "hlv://artifacts/{name}",
                "Artifact",
                "Single global artifact by name",
            ),
            tmpl(
                "hlv://artifacts/milestone/{mid}",
                "Milestone artifacts",
                "Milestone artifact metadata",
            ),
            tmpl(
                "hlv://artifacts/milestone/{mid}/{name}",
                "Milestone artifact",
                "Single milestone artifact",
            ),
        ],
        next_cursor: None,
        meta: None,
    }
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<String, McpError> {
    serde_json::to_string_pretty(value)
        .map_err(|e| McpError::internal_error(format!("JSON serialization error: {e}"), None))
}

fn load_err(what: &str, e: anyhow::Error) -> McpError {
    McpError::internal_error(format!("Failed to load {what}: {e}"), None)
}

fn ok_json(json: String, uri: &str) -> Result<ReadResourceResult, McpError> {
    Ok(ReadResourceResult::new(vec![ResourceContents::text(
        json,
        uri.to_string(),
    )]))
}

pub fn read_resource(root: &Path, uri: &str) -> Result<ReadResourceResult, McpError> {
    let json = match uri {
        "hlv://project" => {
            let pm =
                ProjectMap::load(&root.join("project.yaml")).map_err(|e| load_err("project", e))?;
            to_json(&pm)?
        }
        "hlv://milestones" => {
            let mm = MilestoneMap::load(&root.join("milestones.yaml"))
                .map_err(|e| load_err("milestones", e))?;
            to_json(&mm)?
        }
        "hlv://gates" => {
            let pm =
                ProjectMap::load(&root.join("project.yaml")).map_err(|e| load_err("project", e))?;
            let gp = crate::model::policy::GatesPolicy::load(
                &root.join(&pm.paths.validation.gates_policy),
            )
            .map_err(|e| load_err("gates", e))?;
            to_json(&gp)?
        }
        "hlv://constraints" => {
            let pm =
                ProjectMap::load(&root.join("project.yaml")).map_err(|e| load_err("project", e))?;
            let constraints_dir = root.join(&pm.paths.human.constraints);
            let mut files = Vec::new();
            if constraints_dir.is_dir() {
                for entry in std::fs::read_dir(&constraints_dir)
                    .map_err(|e| load_err("constraints dir", e.into()))?
                {
                    let entry = entry.map_err(|e| load_err("constraints entry", e.into()))?;
                    let path = entry.path();
                    if path
                        .extension()
                        .is_some_and(|ext| ext == "yaml" || ext == "yml")
                    {
                        match crate::model::policy::ConstraintFile::load(&path) {
                            Ok(cf) => files.push(cf),
                            Err(e) => {
                                tracing::warn!("Failed to load constraint {:?}: {e}", path);
                            }
                        }
                    }
                }
            }
            to_json(&files)?
        }
        "hlv://map" => {
            let pm =
                ProjectMap::load(&root.join("project.yaml")).map_err(|e| load_err("project", e))?;
            let map_path = pm.paths.llm.map.as_deref().unwrap_or("llm/map.yaml");
            let map = crate::model::llm_map::LlmMap::load(&root.join(map_path))
                .map_err(|e| load_err("map", e))?;
            to_json(&map)?
        }
        "hlv://workflow" => {
            let data =
                crate::cmd::workflow::get_workflow(root).map_err(|e| load_err("workflow", e))?;
            to_json(&data)?
        }
        "hlv://tasks" => {
            let data = crate::cmd::task::get_task_list(root, None, None, None)
                .map_err(|e| load_err("tasks", e))?;
            to_json(&data)?
        }
        "hlv://artifacts" => {
            let global = ArtifactIndex::load_global(root).map_err(|e| load_err("artifacts", e))?;
            to_json(&global)?
        }
        "hlv://plan" => {
            let mm = MilestoneMap::load(&root.join("milestones.yaml"))
                .map_err(|e| load_err("milestones", e))?;
            match &mm.current {
                Some(current) => {
                    let plan_path = root
                        .join("human/milestones")
                        .join(&current.id)
                        .join("plan.md");
                    if plan_path.exists() {
                        let plan_md = crate::model::plan::PlanMd::load(&plan_path)
                            .map_err(|e| load_err("plan", e))?;
                        to_json(&plan_md)?
                    } else {
                        to_json(&None::<()>)?
                    }
                }
                None => to_json(&None::<()>)?,
            }
        }
        "hlv://traceability" => {
            let pm =
                ProjectMap::load(&root.join("project.yaml")).map_err(|e| load_err("project", e))?;
            let trace_path = pm
                .paths
                .validation
                .traceability
                .as_deref()
                .unwrap_or("validation/traceability.yaml");
            let tm = crate::model::traceability::TraceabilityMap::load(&root.join(trace_path))
                .map_err(|e| load_err("traceability", e))?;
            to_json(&tm)?
        }
        "hlv://glossary" => {
            let pm =
                ProjectMap::load(&root.join("project.yaml")).map_err(|e| load_err("project", e))?;
            let g = Glossary::load(&root.join(&pm.paths.human.glossary))
                .map_err(|e| load_err("glossary", e))?;
            to_json(&g)?
        }
        "hlv://contracts" => {
            return read_contracts_list(root, uri);
        }
        _ => {
            return read_parametrized_resource(root, uri);
        }
    };

    ok_json(json, uri)
}

fn read_contracts_list(root: &Path, uri: &str) -> Result<ReadResourceResult, McpError> {
    let mm =
        MilestoneMap::load(&root.join("milestones.yaml")).map_err(|e| load_err("milestones", e))?;
    let current = mm
        .current
        .as_ref()
        .ok_or_else(|| McpError::invalid_params("No active milestone", None))?;
    let contracts_dir = root.join(format!("human/milestones/{}/contracts", current.id));
    let mut seen = std::collections::BTreeMap::<String, Vec<String>>::new();
    if contracts_dir.is_dir() {
        for entry in
            std::fs::read_dir(&contracts_dir).map_err(|e| load_err("contracts", e.into()))?
        {
            let entry = entry.map_err(|e| load_err("contract entry", e.into()))?;
            let path = entry.path();
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_string();
                seen.entry(stem.to_string()).or_default().push(ext);
            }
        }
    }
    let contracts: Vec<_> = seen
        .into_iter()
        .map(|(id, formats)| {
            serde_json::json!({
                "id": id,
                "formats": formats,
            })
        })
        .collect();
    ok_json(to_json(&contracts)?, uri)
}

fn read_parametrized_resource(root: &Path, uri: &str) -> Result<ReadResourceResult, McpError> {
    // hlv://stage/{n}
    if let Some(n_str) = uri.strip_prefix("hlv://stage/") {
        let n: u32 = n_str.parse().map_err(|_| {
            McpError::invalid_params(format!("Invalid stage number: {n_str}"), None)
        })?;
        let mm = MilestoneMap::load(&root.join("milestones.yaml"))
            .map_err(|e| load_err("milestones", e))?;
        let current = mm
            .current
            .as_ref()
            .ok_or_else(|| McpError::invalid_params("No active milestone", None))?;
        let mid = &current.id;
        let stage_path = root.join(format!("human/milestones/{mid}/stage_{n}.md"));
        if !stage_path.exists() {
            return Err(McpError::resource_not_found(
                format!("Stage {n} not found"),
                None,
            ));
        }
        let stage_plan =
            crate::model::stage::StagePlan::load(&stage_path).map_err(|e| load_err("stage", e))?;
        return ok_json(to_json(&stage_plan)?, uri);
    }

    // hlv://contracts/{id}
    if let Some(id) = uri.strip_prefix("hlv://contracts/") {
        let mm = MilestoneMap::load(&root.join("milestones.yaml"))
            .map_err(|e| load_err("milestones", e))?;
        let current = mm
            .current
            .as_ref()
            .ok_or_else(|| McpError::invalid_params("No active milestone", None))?;
        let contracts_dir = root.join(format!("human/milestones/{}/contracts", current.id));
        let md_path = contracts_dir.join(format!("{id}.md"));
        let yaml_path = contracts_dir.join(format!("{id}.yaml"));

        // Build combined response with available formats
        let mut result = serde_json::Map::new();
        result.insert("id".to_string(), serde_json::json!(id));
        let mut formats = Vec::new();

        if md_path.exists() {
            let text =
                std::fs::read_to_string(&md_path).map_err(|e| load_err("contract", e.into()))?;
            let parsed = crate::model::contract_md::ContractMd::from_markdown(&text);
            result.insert(
                "markdown".to_string(),
                serde_json::to_value(&parsed)
                    .map_err(|e| McpError::internal_error(format!("JSON error: {e}"), None))?,
            );
            formats.push("markdown");
        }
        if yaml_path.exists() {
            let parsed = crate::model::contract_yaml::ContractYaml::load(&yaml_path)
                .map_err(|e| load_err("contract yaml", e))?;
            result.insert(
                "yaml".to_string(),
                serde_json::to_value(&parsed)
                    .map_err(|e| McpError::internal_error(format!("JSON error: {e}"), None))?,
            );
            formats.push("yaml");
        }

        if formats.is_empty() {
            return Err(McpError::resource_not_found(
                format!("Contract '{id}' not found"),
                None,
            ));
        }
        result.insert("formats".to_string(), serde_json::json!(formats));
        return ok_json(to_json(&serde_json::Value::Object(result))?, uri);
    }

    // hlv://tasks/{n}
    if let Some(n_str) = uri.strip_prefix("hlv://tasks/") {
        let n: u32 = n_str.parse().map_err(|_| {
            McpError::invalid_params(format!("Invalid stage number: {n_str}"), None)
        })?;
        let data = crate::cmd::task::get_task_list(root, Some(n), None, None)
            .map_err(|e| load_err("tasks", e))?;
        return ok_json(to_json(&data)?, uri);
    }

    // hlv://artifacts/milestone/{mid}/{name}
    if let Some(rest) = uri.strip_prefix("hlv://artifacts/milestone/") {
        if let Some((mid, name)) = rest.split_once('/') {
            let path = root.join(format!("human/milestones/{mid}/artifacts/{name}.md"));
            if !path.exists() {
                return Err(McpError::resource_not_found(
                    format!("Milestone artifact '{name}' not found in milestone '{mid}'"),
                    None,
                ));
            }
            let artifact = crate::model::artifact::ArtifactFull::load(&path)
                .map_err(|e| load_err("milestone artifact", e))?;
            return ok_json(to_json(&artifact)?, uri);
        } else {
            let mid = rest;
            let artifacts = ArtifactIndex::load_milestone(root, mid)
                .map_err(|e| load_err("milestone artifacts", e))?;
            return ok_json(to_json(&artifacts)?, uri);
        }
    }

    // hlv://artifacts/{name}
    if let Some(name) = uri.strip_prefix("hlv://artifacts/") {
        let path = root.join(format!("human/artifacts/{name}.md"));
        if !path.exists() {
            return Err(McpError::resource_not_found(
                format!("Artifact '{name}' not found"),
                None,
            ));
        }
        let artifact = crate::model::artifact::ArtifactFull::load(&path)
            .map_err(|e| load_err("artifact", e))?;
        return ok_json(to_json(&artifact)?, uri);
    }

    Err(McpError::resource_not_found(
        format!("Unknown resource: {uri}"),
        None,
    ))
}

// ── Workspace (multi-project) resource functions ────────────────────────

/// List resources in workspace mode.
/// Only `hlv://projects` is a fixed resource; all others are templates.
pub fn list_resources_workspace() -> ListResourcesResult {
    ListResourcesResult {
        resources: vec![text_resource(
            "hlv://projects",
            "Projects",
            "List of all projects in the workspace with summary",
        )],
        next_cursor: None,
        meta: None,
    }
}

/// List resource templates in workspace mode.
/// All single-project resources become templates prefixed with `{id}`.
pub fn list_resource_templates_workspace() -> ListResourceTemplatesResult {
    ListResourceTemplatesResult {
        resource_templates: vec![
            tmpl(
                "hlv://projects/{id}",
                "Project",
                "Project configuration and metadata",
            ),
            tmpl(
                "hlv://projects/{id}/milestones",
                "Milestones",
                "All milestones (current + history)",
            ),
            tmpl(
                "hlv://projects/{id}/contracts",
                "Contracts",
                "List of contracts with metadata",
            ),
            tmpl(
                "hlv://projects/{id}/gates",
                "Gates",
                "Validation gates policy",
            ),
            tmpl(
                "hlv://projects/{id}/constraints",
                "Constraints",
                "Project constraints",
            ),
            tmpl("hlv://projects/{id}/map", "Map", "LLM-generated code map"),
            tmpl(
                "hlv://projects/{id}/workflow",
                "Workflow",
                "Current phase and next actions",
            ),
            tmpl(
                "hlv://projects/{id}/tasks",
                "Tasks",
                "All tasks across all stages",
            ),
            tmpl(
                "hlv://projects/{id}/artifacts",
                "Artifacts",
                "Global artifact metadata",
            ),
            tmpl("hlv://projects/{id}/plan", "Plan", "Implementation plan"),
            tmpl(
                "hlv://projects/{id}/traceability",
                "Traceability",
                "Requirement-contract-code traceability",
            ),
            tmpl(
                "hlv://projects/{id}/glossary",
                "Glossary",
                "Domain glossary terms",
            ),
            // Parametrized resources within a project
            tmpl(
                "hlv://projects/{id}/stage/{n}",
                "Stage",
                "Stage plan by number",
            ),
            tmpl(
                "hlv://projects/{id}/contracts/{cid}",
                "Contract",
                "Single contract by ID",
            ),
            tmpl(
                "hlv://projects/{id}/tasks/{n}",
                "Tasks (stage)",
                "Tasks for a specific stage",
            ),
            tmpl(
                "hlv://projects/{id}/artifacts/{name}",
                "Artifact",
                "Single global artifact by name",
            ),
            tmpl(
                "hlv://projects/{id}/artifacts/milestone/{mid}",
                "Milestone artifacts",
                "Milestone artifact metadata",
            ),
            tmpl(
                "hlv://projects/{id}/artifacts/milestone/{mid}/{name}",
                "Milestone artifact",
                "Single milestone artifact",
            ),
        ],
        next_cursor: None,
        meta: None,
    }
}

/// Read a resource in workspace mode.
/// Handles `hlv://projects` (list) and `hlv://projects/{id}/...` (scoped).
pub fn read_resource_workspace(
    mode: &ServerMode,
    uri: &str,
) -> Result<ReadResourceResult, McpError> {
    // hlv://projects — workspace project list
    if uri == "hlv://projects" {
        let config = mode
            .workspace()
            .ok_or_else(|| McpError::internal_error("Not in workspace mode".to_string(), None))?;
        let summaries = config.summaries();
        return ok_json(to_json(&summaries)?, uri);
    }

    // hlv://projects/{id}/... — scoped to a specific project
    if let Some((project_id, inner_uri)) = router::parse_workspace_uri(uri) {
        let root = mode.resolve_root(Some(project_id))?;
        return read_resource(&root, &inner_uri);
    }

    Err(McpError::resource_not_found(
        format!(
            "Unknown workspace resource: {uri}. Use hlv://projects or hlv://projects/{{id}}/..."
        ),
        None,
    ))
}
