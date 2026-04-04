pub mod resources;
pub mod router;
pub mod tools;
pub mod watcher;
pub mod workspace;

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars,
    service::RequestContext,
    tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler,
};
use serde::Deserialize;

use router::ServerMode;

// ── Parameter structs ──────────────────────────────────────────────────

/// Common parameter for tools that only need project_id (in workspace mode).
#[derive(Deserialize, schemars::JsonSchema, Default)]
struct ProjectParam {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct CommitMsgParams {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Whether the current stage is being completed
    stage_complete: Option<bool>,
    /// Override commit type (e.g. feat, fix, chore)
    r#type: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct NameParam {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Name string
    name: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct IdParam {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// ID string
    id: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct OptionalIdParam {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Optional ID (applies to all if omitted)
    id: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct LabelActionParams {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Action: 'add' or 'remove'
    action: String,
    /// Label string
    label: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct MetaActionParams {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Action: 'set' or 'delete'
    action: String,
    /// Metadata key
    key: String,
    /// Metadata value (required for 'set')
    value: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct ConstraintAddParams {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Constraint name
    name: String,
    /// Owner of the constraint
    owner: Option<String>,
    /// Intent/purpose of the constraint
    intent: Option<String>,
    /// What the constraint applies to (e.g. 'backend')
    applies_to: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct ConstraintRuleParams {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Constraint name
    constraint: String,
    /// Rule ID
    rule_id: String,
    /// Severity: critical, high, medium, low
    severity: String,
    /// Rule statement
    statement: String,
    /// Executable command to check this rule (program + args; shell operators and shell variable expansion are not supported)
    check_command: Option<String>,
    /// Working directory for check command (relative to project root)
    check_cwd: Option<String>,
    /// Override diagnostic level for check failures: error, warning, info
    error_level: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct ConstraintRuleRemoveParams {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Constraint name
    constraint: String,
    /// Rule ID to remove
    rule_id: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct ConstraintCheckParams {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Constraint name filter
    constraint: Option<String>,
    /// Rule ID filter
    rule: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct StageLabelParams {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Stage number
    stage_id: u32,
    /// Action: 'add' or 'remove'
    action: String,
    /// Label string
    label: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct StageMetaParams {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Stage number
    stage_id: u32,
    /// Action: 'set' or 'delete'
    action: String,
    /// Metadata key
    key: String,
    /// Metadata value (required for 'set')
    value: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct StageIdParam {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Stage number
    stage_id: u32,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct TaskAddParams {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Stage number to add the task to
    stage_id: u32,
    /// Task ID (e.g. TASK-012 or FIX-001)
    task_id: String,
    /// Task name
    name: String,
    /// Task description (optional, written to stage_N.md)
    description: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct TaskListParams {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Filter by stage number
    stage: Option<u32>,
    /// Filter by status: pending, in_progress, done, blocked
    status: Option<String>,
    /// Filter by label
    label: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct TaskIdParam {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Task ID
    task_id: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct TaskBlockParams {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Task ID to block
    task_id: String,
    /// Reason for blocking
    reason: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct TaskSyncParams {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Force sync even if no changes detected
    force: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct TaskLabelParams {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Task ID
    task_id: String,
    /// Action: 'add' or 'remove'
    action: String,
    /// Label string
    label: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct TaskMetaParams {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Task ID
    task_id: String,
    /// Action: 'set' or 'delete'
    action: String,
    /// Metadata key
    key: String,
    /// Metadata value (required for 'set')
    value: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
struct ArtifactsParams {
    /// Project ID (required in workspace mode, ignored in single-project mode)
    project_id: Option<String>,
    /// Scope: 'global' or 'milestone' (all if omitted)
    scope: Option<String>,
    /// Artifact name to show details (lists all if omitted)
    name: Option<String>,
}

// ── Server ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HlvMcpServer {
    tool_router: ToolRouter<Self>,
    mode: ServerMode,
    subscriptions: watcher::Subscriptions,
    /// Unique ID for this server instance (identifies the connected peer).
    peer_id: u64,
    /// Whether subscriptions/notifications are supported (true only in SSE mode with watcher).
    subscribe_enabled: bool,
}

impl HlvMcpServer {
    /// Resolve the project root for a tool call.
    fn root(&self, project_id: Option<&str>) -> Result<std::path::PathBuf, McpError> {
        self.mode.resolve_root(project_id)
    }
}

#[tool_router]
impl HlvMcpServer {
    /// Create a server for stdio mode (no subscriptions/notifications).
    pub fn new(mode: ServerMode) -> Self {
        Self {
            tool_router: Self::tool_router(),
            mode,
            subscriptions: watcher::new_subscriptions(),
            peer_id: watcher::next_peer_id(),
            subscribe_enabled: false,
        }
    }

    /// Create a server with shared subscriptions (for SSE mode with file watcher).
    pub fn with_subscriptions(mode: ServerMode, subs: watcher::Subscriptions) -> Self {
        Self {
            tool_router: Self::tool_router(),
            mode,
            subscriptions: subs,
            peer_id: watcher::next_peer_id(),
            subscribe_enabled: true,
        }
    }

    #[tool(description = "Run HLV validation checks and return diagnostics")]
    fn hlv_check(
        &self,
        Parameters(p): Parameters<ProjectParam>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_check(&root)
    }

    #[tool(description = "Get current workflow phase, stages, and next recommended actions")]
    fn hlv_workflow(
        &self,
        Parameters(p): Parameters<ProjectParam>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_workflow(&root)
    }

    #[tool(description = "Generate a commit message based on project conventions")]
    fn hlv_commit_msg(
        &self,
        Parameters(p): Parameters<CommitMsgParams>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_commit_msg(
            &root,
            p.stage_complete.unwrap_or(false),
            p.r#type.as_deref(),
        )
    }

    #[tool(description = "Create a new milestone")]
    fn hlv_milestone_new(
        &self,
        Parameters(p): Parameters<NameParam>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_milestone_new(&root, &p.name)
    }

    #[tool(description = "Mark the current milestone as done")]
    fn hlv_milestone_done(
        &self,
        Parameters(p): Parameters<ProjectParam>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_milestone_done(&root)
    }

    #[tool(description = "Abort the current milestone")]
    fn hlv_milestone_abort(
        &self,
        Parameters(p): Parameters<ProjectParam>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_milestone_abort(&root)
    }

    #[tool(description = "Add or remove a label on the current milestone")]
    fn hlv_milestone_label(
        &self,
        Parameters(p): Parameters<LabelActionParams>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_milestone_label(&root, &p.action, &p.label)
    }

    #[tool(description = "Set or delete metadata on the current milestone")]
    fn hlv_milestone_meta(
        &self,
        Parameters(p): Parameters<MetaActionParams>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_milestone_meta(&root, &p.action, &p.key, p.value.as_deref())
    }

    #[tool(description = "Enable a gate by ID")]
    fn hlv_gate_enable(
        &self,
        Parameters(p): Parameters<IdParam>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_gate_enable(&root, &p.id)
    }

    #[tool(description = "Disable a gate by ID")]
    fn hlv_gate_disable(
        &self,
        Parameters(p): Parameters<IdParam>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_gate_disable(&root, &p.id)
    }

    #[tool(description = "Run gate commands and return pass/fail/skip counts")]
    fn hlv_gate_run(
        &self,
        Parameters(p): Parameters<OptionalIdParam>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_gate_run(&root, p.id.as_deref())
    }

    #[tool(description = "Add a new constraint file")]
    fn hlv_constraint_add(
        &self,
        Parameters(p): Parameters<ConstraintAddParams>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_constraint_add(
            &root,
            &p.name,
            p.owner.as_deref(),
            p.intent.as_deref(),
            &p.applies_to,
        )
    }

    #[tool(description = "Remove a constraint file")]
    fn hlv_constraint_remove(
        &self,
        Parameters(p): Parameters<NameParam>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_constraint_remove(&root, &p.name)
    }

    #[tool(description = "Add a rule to an existing constraint")]
    fn hlv_constraint_add_rule(
        &self,
        Parameters(p): Parameters<ConstraintRuleParams>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_constraint_add_rule(
            &root,
            &p.constraint,
            &p.rule_id,
            &p.severity,
            &p.statement,
            p.check_command.as_deref(),
            p.check_cwd.as_deref(),
            p.error_level.as_deref(),
        )
    }

    #[tool(description = "Remove a rule from a constraint")]
    fn hlv_constraint_remove_rule(
        &self,
        Parameters(p): Parameters<ConstraintRuleRemoveParams>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_constraint_remove_rule(&root, &p.constraint, &p.rule_id)
    }

    #[tool(description = "Run check commands for constraint rules and return pass/fail results")]
    fn hlv_constraint_check(
        &self,
        Parameters(p): Parameters<ConstraintCheckParams>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_constraint_check(&root, p.constraint.as_deref(), p.rule.as_deref())
    }

    #[tool(description = "Add or remove a label on a stage")]
    fn hlv_stage_label(
        &self,
        Parameters(p): Parameters<StageLabelParams>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_stage_label(&root, p.stage_id, &p.action, &p.label)
    }

    #[tool(description = "Set or delete metadata on a stage")]
    fn hlv_stage_meta(
        &self,
        Parameters(p): Parameters<StageMetaParams>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_stage_meta(&root, p.stage_id, &p.action, &p.key, p.value.as_deref())
    }

    #[tool(
        description = "Reopen a stage: implemented→implementing, validated→validating, validating→implementing"
    )]
    fn hlv_stage_reopen(
        &self,
        Parameters(p): Parameters<StageIdParam>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_stage_reopen(&root, p.stage_id)
    }

    #[tool(
        description = "Add a new task to a stage. Auto-reopens the stage if it is implemented/validated/validating"
    )]
    fn hlv_task_add(
        &self,
        Parameters(p): Parameters<TaskAddParams>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_task_add(
            &root,
            p.stage_id,
            &p.task_id,
            &p.name,
            p.description.as_deref(),
        )
    }

    #[tool(description = "List tasks with optional filters by stage, status, or label")]
    fn hlv_task_list(
        &self,
        Parameters(p): Parameters<TaskListParams>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_task_list(&root, p.stage, p.status.as_deref(), p.label.as_deref())
    }

    #[tool(description = "Start a task (transition to in_progress)")]
    fn hlv_task_start(
        &self,
        Parameters(p): Parameters<TaskIdParam>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_task_start(&root, &p.task_id)
    }

    #[tool(description = "Mark a task as done")]
    fn hlv_task_done(
        &self,
        Parameters(p): Parameters<TaskIdParam>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_task_done(&root, &p.task_id)
    }

    #[tool(description = "Block a task with a reason")]
    fn hlv_task_block(
        &self,
        Parameters(p): Parameters<TaskBlockParams>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_task_block(&root, &p.task_id, &p.reason)
    }

    #[tool(description = "Unblock a previously blocked task")]
    fn hlv_task_unblock(
        &self,
        Parameters(p): Parameters<TaskIdParam>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_task_unblock(&root, &p.task_id)
    }

    #[tool(description = "Sync tasks from stage plan files into milestones.yaml")]
    fn hlv_task_sync(
        &self,
        Parameters(p): Parameters<TaskSyncParams>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_task_sync(&root, p.force.unwrap_or(false))
    }

    #[tool(description = "Add or remove a label on a task")]
    fn hlv_task_label(
        &self,
        Parameters(p): Parameters<TaskLabelParams>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_task_label(&root, &p.task_id, &p.action, &p.label)
    }

    #[tool(description = "Set or delete metadata on a task")]
    fn hlv_task_meta(
        &self,
        Parameters(p): Parameters<TaskMetaParams>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_task_meta(&root, &p.task_id, &p.action, &p.key, p.value.as_deref())
    }

    #[tool(description = "List or show artifacts. Use scope to filter by global/milestone")]
    fn hlv_artifacts(
        &self,
        Parameters(p): Parameters<ArtifactsParams>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_artifacts(&root, p.scope.as_deref(), p.name.as_deref())
    }

    #[tool(description = "Display the project glossary of domain terms")]
    fn hlv_glossary(
        &self,
        Parameters(p): Parameters<ProjectParam>,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root(p.project_id.as_deref())?;
        tools::hlv_glossary(&root)
    }
}

#[tool_handler]
impl ServerHandler for HlvMcpServer {
    fn get_info(&self) -> ServerInfo {
        let mut caps = ServerCapabilities::builder()
            .enable_tools()
            .enable_resources();
        if self.subscribe_enabled {
            caps = caps.enable_resources_subscribe();
        }
        let instructions = if self.mode.is_workspace() {
            "HLV (Human-Led Validation) MCP server in WORKSPACE mode. \
             Multiple projects available. Use hlv://projects to list them. \
             Resources use hlv://projects/{id}/... URIs. \
             Tools require project_id parameter."
        } else {
            "HLV (Human-Led Validation) MCP server. \
             Provides read-only access to project data (resources) \
             and operations (tools) for HLV projects."
        };
        ServerInfo::new(caps.build()).with_instructions(instructions.to_string())
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        if self.mode.is_workspace() {
            Ok(resources::list_resources_workspace())
        } else {
            Ok(resources::list_resources())
        }
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let uri = request.uri.to_string();
        if self.mode.is_workspace() {
            resources::read_resource_workspace(&self.mode, &uri)
        } else {
            // Single mode: resolve root once (project_id not needed)
            let root = self.mode.resolve_root(None)?;
            resources::read_resource(&root, &uri)
        }
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        if self.mode.is_workspace() {
            Ok(resources::list_resource_templates_workspace())
        } else {
            Ok(resources::list_resource_templates())
        }
    }

    async fn subscribe(
        &self,
        request: SubscribeRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        watcher::subscribe(
            &self.subscriptions,
            request.uri.to_string(),
            ctx.peer,
            self.peer_id,
        )
        .await;
        Ok(())
    }

    async fn unsubscribe(
        &self,
        request: UnsubscribeRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        watcher::unsubscribe(&self.subscriptions, &request.uri.to_string(), self.peer_id).await;
        Ok(())
    }
}
