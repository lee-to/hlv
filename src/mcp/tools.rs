use rmcp::{
    model::{CallToolResult, Content},
    ErrorData as McpError,
};
use std::path::Path;

/// Helper: convert anyhow error to MCP internal error
fn mcp_err(context: &str, e: anyhow::Error) -> McpError {
    McpError::internal_error(format!("{context}: {e}"), None)
}

/// Helper: serialize value to pretty JSON and wrap in CallToolResult
fn json_ok(value: &impl serde::Serialize) -> Result<CallToolResult, McpError> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| McpError::internal_error(format!("JSON error: {e}"), None))?;
    Ok(CallToolResult::success(vec![Content::text(text)]))
}

/// Helper: wrap a simple success message
fn text_ok(msg: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![Content::text(msg.into())]))
}

/// Execute a closure with stdout suppressed (quiet mode).
/// Prevents CLI functions from printing to stdout, which is the JSON-RPC channel
/// in stdio transport.
fn quiet<T>(f: impl FnOnce() -> T) -> T {
    crate::cmd::style::set_quiet(true);
    let result = f();
    crate::cmd::style::set_quiet(false);
    result
}

// ── hlv_check ──────────────────────────────────────────────────────────

pub fn hlv_check(root: &Path) -> Result<CallToolResult, McpError> {
    let (diagnostics, exit_code) =
        crate::cmd::check::get_check_diagnostics(root).map_err(|e| mcp_err("Check failed", e))?;

    json_ok(&serde_json::json!({
        "exit_code": exit_code,
        "diagnostics": diagnostics,
    }))
}

// ── hlv_workflow ───────────────────────────────────────────────────────

pub fn hlv_workflow(root: &Path) -> Result<CallToolResult, McpError> {
    let data =
        crate::cmd::workflow::get_workflow(root).map_err(|e| mcp_err("Workflow failed", e))?;
    json_ok(&data)
}

// ── hlv_commit_msg ────────────────────────────────────────────────────

pub fn hlv_commit_msg(
    root: &Path,
    stage_complete: bool,
    type_override: Option<&str>,
) -> Result<CallToolResult, McpError> {
    let msg = crate::cmd::commit_msg::get_commit_msg(root, stage_complete, type_override)
        .map_err(|e| mcp_err("commit-msg failed", e))?;
    json_ok(&serde_json::json!({ "message": msg }))
}

// ── Milestone tools ───────────────────────────────────────────────────

pub fn hlv_milestone_new(root: &Path, name: &str) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::milestone::run_new(root, name))
        .map_err(|e| mcp_err("milestone new failed", e))?;
    text_ok(format!("Milestone '{name}' created"))
}

pub fn hlv_milestone_done(root: &Path) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::milestone::run_done(root))
        .map_err(|e| mcp_err("milestone done failed", e))?;
    text_ok("Milestone marked as done")
}

pub fn hlv_milestone_abort(root: &Path) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::milestone::run_abort(root))
        .map_err(|e| mcp_err("milestone abort failed", e))?;
    text_ok("Milestone aborted")
}

pub fn hlv_milestone_label(
    root: &Path,
    action: &str,
    label: &str,
) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::stage::run_milestone_label(root, action, label))
        .map_err(|e| mcp_err("milestone label failed", e))?;
    text_ok(format!("Label '{label}' {action}d on milestone"))
}

pub fn hlv_milestone_meta(
    root: &Path,
    action: &str,
    key: &str,
    value: Option<&str>,
) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::stage::run_milestone_meta(root, action, key, value))
        .map_err(|e| mcp_err("milestone meta failed", e))?;
    text_ok(format!("Milestone meta key '{key}' {action}"))
}

// ── Gate tools ────────────────────────────────────────────────────────

pub fn hlv_gate_enable(root: &Path, id: &str) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::gates::run_enable(root, id))
        .map_err(|e| mcp_err("gate enable failed", e))?;
    text_ok(format!("Gate '{id}' enabled"))
}

pub fn hlv_gate_disable(root: &Path, id: &str) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::gates::run_disable(root, id))
        .map_err(|e| mcp_err("gate disable failed", e))?;
    text_ok(format!("Gate '{id}' disabled"))
}

pub fn hlv_gate_run(root: &Path, id: Option<&str>) -> Result<CallToolResult, McpError> {
    let (passed, failed, skipped) = quiet(|| crate::cmd::gates::run_gate_commands(root, id))
        .map_err(|e| mcp_err("gate run failed", e))?;
    json_ok(&serde_json::json!({
        "passed": passed,
        "failed": failed,
        "skipped": skipped,
    }))
}

// ── Constraint tools ──────────────────────────────────────────────────

pub fn hlv_constraint_add(
    root: &Path,
    name: &str,
    owner: Option<&str>,
    intent: Option<&str>,
    applies_to: &str,
) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::constraints::run_add(root, name, owner, intent, applies_to))
        .map_err(|e| mcp_err("constraint add failed", e))?;
    text_ok(format!("Constraint '{name}' added"))
}

pub fn hlv_constraint_remove(root: &Path, name: &str) -> Result<CallToolResult, McpError> {
    // force=true: MCP tools must be non-interactive (no stdin prompt)
    quiet(|| crate::cmd::constraints::run_remove(root, name, true))
        .map_err(|e| mcp_err("constraint remove failed", e))?;
    text_ok(format!("Constraint '{name}' removed"))
}

#[allow(clippy::too_many_arguments)]
pub fn hlv_constraint_add_rule(
    root: &Path,
    constraint: &str,
    rule_id: &str,
    severity: &str,
    statement: &str,
    check_command: Option<&str>,
    check_cwd: Option<&str>,
    error_level: Option<&str>,
) -> Result<CallToolResult, McpError> {
    quiet(|| {
        crate::cmd::constraints::run_add_rule(
            root,
            constraint,
            rule_id,
            severity,
            statement,
            check_command,
            check_cwd,
            error_level,
        )
    })
    .map_err(|e| mcp_err("constraint add-rule failed", e))?;
    text_ok(format!(
        "Rule '{rule_id}' added to constraint '{constraint}'"
    ))
}

pub fn hlv_constraint_check(
    root: &Path,
    constraint: Option<&str>,
    rule: Option<&str>,
) -> Result<CallToolResult, McpError> {
    let data = crate::cmd::constraints::get_constraint_check_results(root, constraint, rule)
        .map_err(|e| mcp_err("constraint check failed", e))?;
    json_ok(&data)
}

pub fn hlv_constraint_remove_rule(
    root: &Path,
    constraint: &str,
    rule_id: &str,
) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::constraints::run_remove_rule(root, constraint, rule_id))
        .map_err(|e| mcp_err("constraint remove-rule failed", e))?;
    text_ok(format!(
        "Rule '{rule_id}' removed from constraint '{constraint}'"
    ))
}

// ── Stage tools ───────────────────────────────────────────────────────

pub fn hlv_stage_label(
    root: &Path,
    stage_id: u32,
    action: &str,
    label: &str,
) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::stage::run_label(root, stage_id, action, label))
        .map_err(|e| mcp_err("stage label failed", e))?;
    text_ok(format!("Label '{label}' {action}d on stage {stage_id}"))
}

pub fn hlv_stage_meta(
    root: &Path,
    stage_id: u32,
    action: &str,
    key: &str,
    value: Option<&str>,
) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::stage::run_meta(root, stage_id, action, key, value))
        .map_err(|e| mcp_err("stage meta failed", e))?;
    text_ok(format!("Stage {stage_id} meta key '{key}' {action}"))
}

// ── Stage reopen ─────────────────────────────────────────────────────

pub fn hlv_stage_reopen(root: &Path, stage_id: u32) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::stage::run_reopen(root, stage_id))
        .map_err(|e| mcp_err("stage reopen failed", e))?;
    text_ok(format!("Stage {stage_id} reopened"))
}

// ── Task tools ────────────────────────────────────────────────────────

pub fn hlv_task_add(
    root: &Path,
    stage_id: u32,
    task_id: &str,
    name: &str,
    description: Option<&str>,
) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::task::run_add(root, stage_id, task_id, name, description))
        .map_err(|e| mcp_err("task add failed", e))?;
    text_ok(format!("Task '{task_id}' added to stage {stage_id}"))
}

pub fn hlv_task_list(
    root: &Path,
    stage: Option<u32>,
    status: Option<&str>,
    label: Option<&str>,
) -> Result<CallToolResult, McpError> {
    let tasks = crate::cmd::task::get_task_list(root, stage, status, label)
        .map_err(|e| mcp_err("task list failed", e))?;
    json_ok(&tasks)
}

pub fn hlv_task_start(root: &Path, task_id: &str) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::task::run_start(root, task_id))
        .map_err(|e| mcp_err("task start failed", e))?;
    text_ok(format!("Task '{task_id}' started"))
}

pub fn hlv_task_done(root: &Path, task_id: &str) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::task::run_done(root, task_id))
        .map_err(|e| mcp_err("task done failed", e))?;
    text_ok(format!("Task '{task_id}' marked as done"))
}

pub fn hlv_task_block(
    root: &Path,
    task_id: &str,
    reason: &str,
) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::task::run_block(root, task_id, reason))
        .map_err(|e| mcp_err("task block failed", e))?;
    text_ok(format!("Task '{task_id}' blocked: {reason}"))
}

pub fn hlv_task_unblock(root: &Path, task_id: &str) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::task::run_unblock(root, task_id))
        .map_err(|e| mcp_err("task unblock failed", e))?;
    text_ok(format!("Task '{task_id}' unblocked"))
}

pub fn hlv_task_sync(root: &Path, force: bool) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::task::run_sync(root, force))
        .map_err(|e| mcp_err("task sync failed", e))?;
    text_ok("Tasks synced from stage plans")
}

pub fn hlv_task_label(
    root: &Path,
    task_id: &str,
    action: &str,
    label: &str,
) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::task::run_label(root, task_id, action, label))
        .map_err(|e| mcp_err("task label failed", e))?;
    text_ok(format!("Label '{label}' {action}d on task '{task_id}'"))
}

pub fn hlv_task_meta(
    root: &Path,
    task_id: &str,
    action: &str,
    key: &str,
    value: Option<&str>,
) -> Result<CallToolResult, McpError> {
    quiet(|| crate::cmd::task::run_meta(root, task_id, action, key, value))
        .map_err(|e| mcp_err("task meta failed", e))?;
    text_ok(format!("Task '{task_id}' meta key '{key}' {action}"))
}

// ── Artifacts tool ────────────────────────────────────────────────────

pub fn hlv_artifacts(
    root: &Path,
    scope: Option<&str>,
    name: Option<&str>,
) -> Result<CallToolResult, McpError> {
    let (global_only, milestone_only) = match scope {
        Some("global") => (true, false),
        Some("milestone") => (false, true),
        _ => (false, false),
    };

    match name {
        Some(n) => {
            let artifact =
                crate::cmd::artifacts::get_artifact_show(root, n, global_only, milestone_only)
                    .map_err(|e| mcp_err("artifacts show failed", e))?;
            json_ok(&artifact)
        }
        None => {
            let index =
                crate::cmd::artifacts::get_artifacts_list(root, global_only, milestone_only)
                    .map_err(|e| mcp_err("artifacts list failed", e))?;
            json_ok(&index)
        }
    }
}

// ── Glossary tool ─────────────────────────────────────────────────────

pub fn hlv_glossary(root: &Path) -> Result<CallToolResult, McpError> {
    let glossary =
        crate::cmd::glossary::get_glossary(root).map_err(|e| mcp_err("glossary failed", e))?;
    json_ok(&glossary)
}
