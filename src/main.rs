use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "hlv", version, about = "HLV — Human-Led Validation toolkit")]
struct Cli {
    /// Path to project root (searches for project.yaml)
    #[arg(long, global = true)]
    root: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scaffold a new HLV project (prompts interactively if options omitted)
    Init {
        /// Project name
        #[arg(long)]
        project: Option<String>,
        /// Owner team
        #[arg(long)]
        owner: Option<String>,
        /// Agent name (creates .{agent}/skills/)
        #[arg(long)]
        agent: Option<String>,
        /// Gate profile: minimal, standard, full (prompts if omitted)
        #[arg(long)]
        profile: Option<String>,
        /// Target directory
        #[arg(long, default_value = ".")]
        path: String,
    },
    /// Validate project artifacts
    Check {
        /// Watch for changes and re-validate
        #[arg(long)]
        watch: bool,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Show project status
    Status {
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Show implementation plan
    Plan {
        /// Render ASCII dependency graph
        #[arg(long)]
        visual: bool,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Show traceability map
    Trace {
        /// Render ASCII traceability graph
        #[arg(long)]
        visual: bool,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Manage validation gates
    Gates {
        /// Output in JSON format
        #[arg(long)]
        json: bool,

        #[command(subcommand)]
        action: Option<GatesAction>,
    },
    /// Manage constraints
    Constraints {
        #[command(subcommand)]
        action: Option<ConstraintsAction>,

        /// Filter by severity (critical, high, medium, low)
        #[arg(long)]
        severity: Option<String>,

        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Generate commit message based on git policy and current milestone
    CommitMsg {
        /// Mark as stage complete
        #[arg(long)]
        stage: bool,
        /// Override commit type (feat, fix, refactor, test, docs, chore)
        #[arg(long, rename_all = "snake_case")]
        r#type: Option<String>,
    },
    /// Launch interactive TUI dashboard
    Dashboard,
    /// Show workflow diagram and next steps
    Workflow {
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Show project glossary
    Glossary {
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Manage milestones
    Milestone {
        #[command(subcommand)]
        action: MilestoneAction,
    },
    /// Manage task statuses
    Task {
        #[command(subcommand)]
        action: TaskAction,
    },
    /// List and show project artifacts
    Artifacts {
        #[command(subcommand)]
        action: Option<ArtifactsAction>,

        /// Show only global artifacts
        #[arg(long)]
        global: bool,

        /// Show only milestone artifacts
        #[arg(long)]
        milestone: bool,

        /// Output in JSON
        #[arg(long)]
        json: bool,
    },
    /// Manage stage labels and metadata
    Stage {
        #[command(subcommand)]
        action: StageAction,
    },
    /// Update hlv to the latest version from GitHub Releases
    Update {
        /// Only check for updates, don't install
        #[arg(long)]
        check: bool,
    },
    /// Manage MCP workspace (multi-project config)
    Workspace {
        #[command(subcommand)]
        action: WorkspaceAction,

        /// Path to workspace config (default: ~/.hlv/workspace.yaml)
        #[arg(long, global = true)]
        config: Option<String>,
    },
    /// Start MCP (Model Context Protocol) server
    Mcp {
        /// Transport: stdio (default) or sse
        #[arg(long, default_value = "stdio")]
        transport: String,
        /// Port for SSE transport
        #[arg(long, default_value = "3000")]
        port: u16,
        /// Path to workspace config YAML for multi-project mode
        #[arg(long)]
        workspace: Option<String>,
    },
}

#[derive(Subcommand)]
enum MilestoneAction {
    /// Create a new milestone
    New {
        /// Milestone name (e.g. "add-payments")
        name: String,
    },
    /// Show current milestone status
    Status,
    /// List all milestones (current + history)
    List,
    /// Complete current milestone (all stages must be validated)
    Done,
    /// Abort current milestone
    Abort,
    /// Manage milestone labels
    Label {
        /// Action: add or remove
        action: String,
        /// Label value
        label: String,
    },
    /// Manage milestone metadata
    Meta {
        /// Action: set or delete
        action: String,
        /// Key
        key: String,
        /// Value (required for 'set')
        value: Option<String>,
    },
}

#[derive(Subcommand)]
enum TaskAction {
    /// List tasks
    List {
        /// Filter by stage number
        #[arg(long)]
        stage: Option<u32>,
        /// Filter by status (pending, in_progress, done, blocked)
        #[arg(long)]
        status: Option<String>,
        /// Filter by label
        #[arg(long)]
        label: Option<String>,
        /// Output in JSON
        #[arg(long)]
        json: bool,
    },
    /// Start a task
    Start {
        /// Task ID (e.g. TASK-001)
        id: String,
    },
    /// Mark a task as done
    Done {
        /// Task ID
        id: String,
    },
    /// Block a task
    Block {
        /// Task ID
        id: String,
        /// Reason for blocking
        #[arg(long)]
        reason: String,
    },
    /// Unblock a task
    Unblock {
        /// Task ID
        id: String,
    },
    /// Show task status summary
    Status {
        /// Output in JSON
        #[arg(long)]
        json: bool,
    },
    /// Add a new task to a stage
    Add {
        /// Task ID (e.g. TASK-012)
        id: String,
        /// Task name
        name: String,
        /// Stage number
        #[arg(long)]
        stage: u32,
        /// Task description
        #[arg(long)]
        description: Option<String>,
    },
    /// Sync tasks from stage_N.md plans
    Sync {
        /// Force removal of active tasks not in plan
        #[arg(long)]
        force: bool,
    },
    /// Manage task labels
    Label {
        /// Task ID
        id: String,
        /// Action: add or remove
        action: String,
        /// Label value
        label: String,
    },
    /// Manage task metadata
    Meta {
        /// Task ID
        id: String,
        /// Action: set or delete
        action: String,
        /// Key
        key: String,
        /// Value (required for 'set')
        value: Option<String>,
    },
}

#[derive(Subcommand)]
enum ArtifactsAction {
    /// Show a specific artifact
    Show {
        /// Artifact name (e.g. context, stack)
        name: String,
        /// Show only global artifacts
        #[arg(long)]
        global: bool,
        /// Show only milestone artifacts
        #[arg(long)]
        milestone: bool,
        /// Output in JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum StageAction {
    /// Reopen a stage (revert implemented→implementing, validated→validating)
    Reopen {
        /// Stage number
        id: u32,
    },
    /// Manage stage labels
    Label {
        /// Stage number
        id: u32,
        /// Action: add or remove
        action: String,
        /// Label value
        label: String,
    },
    /// Manage stage metadata
    Meta {
        /// Stage number
        id: u32,
        /// Action: set or delete
        action: String,
        /// Key
        key: String,
        /// Value (required for 'set')
        value: Option<String>,
    },
}

#[derive(Subcommand)]
enum WorkspaceAction {
    /// Initialize workspace config file
    Init,
    /// Add a project to the workspace
    Add {
        /// Project ID (defaults to directory name)
        id: Option<String>,
        /// Project root path (defaults to current directory)
        #[arg(long)]
        root: Option<String>,
    },
    /// Remove a project from the workspace
    Remove {
        /// Project ID to remove
        id: String,
    },
    /// List projects in the workspace
    List,
}

#[derive(Subcommand)]
enum GatesAction {
    /// Enable a gate
    Enable {
        /// Gate ID (e.g. GATE-CONTRACT-001)
        id: String,
    },
    /// Disable a gate
    Disable {
        /// Gate ID (e.g. GATE-CONTRACT-001)
        id: String,
    },
    /// Set the shell command for a gate
    SetCmd {
        /// Gate ID
        id: String,
        /// Shell command to run (e.g. "cargo test")
        command: String,
    },
    /// Clear the command for a gate
    ClearCmd {
        /// Gate ID
        id: String,
    },
    /// Set working directory for a gate (relative to project root)
    SetCwd {
        /// Gate ID
        id: String,
        /// Directory relative to project root (e.g. "llm")
        cwd: String,
    },
    /// Clear working directory for a gate (resets to project root)
    ClearCwd {
        /// Gate ID
        id: String,
    },
    /// Run all enabled gates (or a single gate by ID)
    Run {
        /// Gate ID to run (runs all if omitted)
        id: Option<String>,
    },
    /// Add a new gate
    Add {
        /// Gate ID (e.g. GATE-LINT-001)
        id: String,
        /// Gate type (contract_tests, integration_tests, security, lint, performance, mutation_testing, observability, custom)
        #[arg(long, rename_all = "snake_case")]
        r#type: String,
        /// Mark gate as mandatory
        #[arg(long)]
        mandatory: bool,
        /// Shell command to run
        #[arg(long)]
        command: Option<String>,
        /// Working directory relative to project root
        #[arg(long)]
        cwd: Option<String>,
        /// Create gate as disabled
        #[arg(long)]
        no_enable: bool,
    },
    /// Remove a gate
    Remove {
        /// Gate ID
        id: String,
        /// Skip confirmation for mandatory gates
        #[arg(long)]
        force: bool,
    },
    /// Edit gate properties (type, mandatory)
    Edit {
        /// Gate ID
        id: String,
        /// New gate type
        #[arg(long, rename_all = "snake_case")]
        r#type: Option<String>,
        /// Set mandatory flag
        #[arg(long)]
        mandatory: bool,
        /// Clear mandatory flag
        #[arg(long)]
        no_mandatory: bool,
    },
}

#[derive(Subcommand)]
enum ConstraintsAction {
    /// List all constraints and rules
    List {
        /// Filter by severity
        #[arg(long)]
        severity: Option<String>,
        /// Output in JSON
        #[arg(long)]
        json: bool,
    },
    /// Show details of a constraint
    Show {
        /// Constraint name (e.g. security)
        name: String,
        /// Output in JSON
        #[arg(long)]
        json: bool,
    },
    /// Add a new constraint
    Add {
        /// Constraint name (e.g. observability)
        name: String,
        /// Owner team
        #[arg(long)]
        owner: Option<String>,
        /// Purpose description
        #[arg(long)]
        intent: Option<String>,
        /// Scope (default: global)
        #[arg(long, default_value = "global")]
        applies_to: String,
    },
    /// Remove a constraint
    Remove {
        /// Constraint name
        name: String,
        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },
    /// Add a rule to a constraint
    AddRule {
        /// Constraint name (e.g. security)
        constraint: String,
        /// Rule ID (e.g. rate_limit_per_ip)
        rule_id: String,
        /// Severity: critical, high, medium, low
        #[arg(long)]
        severity: String,
        /// Rule statement
        #[arg(long)]
        statement: String,
    },
    /// Remove a rule from a constraint
    RemoveRule {
        /// Constraint name
        constraint: String,
        /// Rule ID
        rule_id: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = run(cli);

    if let Err(e) = result {
        hlv::cmd::style::fatal(&hlv::cmd::style::format_error(&e));
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    // Init doesn't need an existing project root
    if let Commands::Init {
        project,
        owner,
        agent,
        profile,
        path,
    } = cli.command
    {
        return hlv::cmd::init::run(
            &path,
            project.as_deref(),
            owner.as_deref(),
            agent.as_deref(),
            profile.as_deref(),
        );
    }

    // Update doesn't need a project root
    if let Commands::Update { check } = cli.command {
        return hlv::cmd::update::run(check);
    }

    // Workspace commands don't need a project root
    if let Commands::Workspace { action, config } = cli.command {
        return match action {
            WorkspaceAction::Init => hlv::cmd::workspace::run_init(config.as_deref()),
            WorkspaceAction::Add { id, root } => {
                hlv::cmd::workspace::run_add(id.as_deref(), root.as_deref(), config.as_deref())
            }
            WorkspaceAction::Remove { id } => {
                hlv::cmd::workspace::run_remove(&id, config.as_deref())
            }
            WorkspaceAction::List => hlv::cmd::workspace::run_list(config.as_deref()),
        };
    }

    // MCP may run in workspace mode (no project root needed)
    if let Commands::Mcp {
        transport,
        port,
        workspace,
    } = &cli.command
    {
        let project_root = if workspace.is_some() {
            None
        } else {
            Some(hlv::find_project_root(cli.root.as_deref())?)
        };
        return hlv::cmd::mcp::run(
            project_root.as_deref(),
            workspace.as_deref(),
            transport,
            *port,
        );
    }

    let project_root = hlv::find_project_root(cli.root.as_deref())?;

    match cli.command {
        Commands::Init { .. } => unreachable!(),
        Commands::Check { watch, json } => hlv::cmd::check::run(&project_root, watch, json),
        Commands::Status { json } => hlv::cmd::status::run(&project_root, json),
        Commands::Plan { visual, json } => hlv::cmd::plan::run(&project_root, visual, json),
        Commands::Trace { visual, json } => hlv::cmd::trace::run(&project_root, visual, json),
        Commands::Gates { json, action } => match action {
            None => {
                if json {
                    hlv::cmd::gates::run_show_json(&project_root)
                } else {
                    hlv::cmd::gates::run(&project_root)
                }
            }
            Some(GatesAction::Enable { id }) => hlv::cmd::gates::run_enable(&project_root, &id),
            Some(GatesAction::Disable { id }) => hlv::cmd::gates::run_disable(&project_root, &id),
            Some(GatesAction::SetCmd { id, command }) => {
                hlv::cmd::gates::run_set_command(&project_root, &id, &command)
            }
            Some(GatesAction::ClearCmd { id }) => {
                hlv::cmd::gates::run_clear_command(&project_root, &id)
            }
            Some(GatesAction::SetCwd { id, cwd }) => {
                hlv::cmd::gates::run_set_cwd(&project_root, &id, &cwd)
            }
            Some(GatesAction::ClearCwd { id }) => {
                hlv::cmd::gates::run_clear_cwd(&project_root, &id)
            }
            Some(GatesAction::Run { id }) => {
                let (_, failed, _) =
                    hlv::cmd::gates::run_gate_commands(&project_root, id.as_deref())?;
                if failed > 0 {
                    std::process::exit(1);
                }
                Ok(())
            }
            Some(GatesAction::Add {
                id,
                r#type,
                mandatory,
                command,
                cwd,
                no_enable,
            }) => hlv::cmd::gates::run_add(
                &project_root,
                &id,
                &r#type,
                mandatory,
                command.as_deref(),
                cwd.as_deref(),
                !no_enable,
            ),
            Some(GatesAction::Remove { id, force }) => {
                hlv::cmd::gates::run_remove(&project_root, &id, force)
            }
            Some(GatesAction::Edit {
                id,
                r#type,
                mandatory,
                no_mandatory,
            }) => hlv::cmd::gates::run_edit(
                &project_root,
                &id,
                r#type.as_deref(),
                mandatory,
                no_mandatory,
            ),
        },
        Commands::Constraints {
            action,
            severity,
            json,
        } => match action {
            None => hlv::cmd::constraints::run_list(&project_root, severity.as_deref(), json),
            Some(ConstraintsAction::List {
                severity: sev,
                json: j,
            }) => hlv::cmd::constraints::run_list(
                &project_root,
                sev.as_deref().or(severity.as_deref()),
                j || json,
            ),
            Some(ConstraintsAction::Show { name, json: j }) => {
                hlv::cmd::constraints::run_show(&project_root, &name, j || json)
            }
            Some(ConstraintsAction::Add {
                name,
                owner,
                intent,
                applies_to,
            }) => hlv::cmd::constraints::run_add(
                &project_root,
                &name,
                owner.as_deref(),
                intent.as_deref(),
                &applies_to,
            ),
            Some(ConstraintsAction::Remove { name, force }) => {
                hlv::cmd::constraints::run_remove(&project_root, &name, force)
            }
            Some(ConstraintsAction::AddRule {
                constraint,
                rule_id,
                severity: sev,
                statement,
            }) => hlv::cmd::constraints::run_add_rule(
                &project_root,
                &constraint,
                &rule_id,
                &sev,
                &statement,
            ),
            Some(ConstraintsAction::RemoveRule {
                constraint,
                rule_id,
            }) => hlv::cmd::constraints::run_remove_rule(&project_root, &constraint, &rule_id),
        },
        Commands::CommitMsg { stage, r#type } => {
            hlv::cmd::commit_msg::run(&project_root, stage, r#type.as_deref())
        }
        Commands::Dashboard => hlv::cmd::dashboard::run(&project_root),
        Commands::Workflow { json } => hlv::cmd::workflow::run(&project_root, json),
        Commands::Glossary { json } => hlv::cmd::glossary::run(&project_root, json),
        Commands::Milestone { action } => match action {
            MilestoneAction::New { name } => hlv::cmd::milestone::run_new(&project_root, &name),
            MilestoneAction::Status => hlv::cmd::milestone::run_status(&project_root),
            MilestoneAction::List => hlv::cmd::milestone::run_list(&project_root),
            MilestoneAction::Done => hlv::cmd::milestone::run_done(&project_root),
            MilestoneAction::Abort => hlv::cmd::milestone::run_abort(&project_root),
            MilestoneAction::Label { action, label } => {
                hlv::cmd::stage::run_milestone_label(&project_root, &action, &label)
            }
            MilestoneAction::Meta { action, key, value } => {
                hlv::cmd::stage::run_milestone_meta(&project_root, &action, &key, value.as_deref())
            }
        },
        Commands::Task { action } => match action {
            TaskAction::List {
                stage,
                status,
                label,
                json,
            } => hlv::cmd::task::run_list(
                &project_root,
                stage,
                status.as_deref(),
                label.as_deref(),
                json,
            ),
            TaskAction::Add {
                id,
                name,
                stage,
                description,
            } => hlv::cmd::task::run_add(&project_root, stage, &id, &name, description.as_deref()),
            TaskAction::Start { id } => hlv::cmd::task::run_start(&project_root, &id),
            TaskAction::Done { id } => hlv::cmd::task::run_done(&project_root, &id),
            TaskAction::Block { id, reason } => {
                hlv::cmd::task::run_block(&project_root, &id, &reason)
            }
            TaskAction::Unblock { id } => hlv::cmd::task::run_unblock(&project_root, &id),
            TaskAction::Status { json } => hlv::cmd::task::run_status(&project_root, json),
            TaskAction::Sync { force } => hlv::cmd::task::run_sync(&project_root, force),
            TaskAction::Label { id, action, label } => {
                hlv::cmd::task::run_label(&project_root, &id, &action, &label)
            }
            TaskAction::Meta {
                id,
                action,
                key,
                value,
            } => hlv::cmd::task::run_meta(&project_root, &id, &action, &key, value.as_deref()),
        },
        Commands::Artifacts {
            action,
            global,
            milestone,
            json,
        } => match action {
            None => hlv::cmd::artifacts::run_list(&project_root, global, milestone, json),
            Some(ArtifactsAction::Show {
                name,
                global: g,
                milestone: m,
                json: j,
            }) => hlv::cmd::artifacts::run_show(
                &project_root,
                &name,
                g || global,
                m || milestone,
                j || json,
            ),
        },
        Commands::Mcp { .. } => unreachable!(),
        Commands::Update { .. } => unreachable!(),
        Commands::Workspace { .. } => unreachable!(),
        Commands::Stage { action } => match action {
            StageAction::Reopen { id } => hlv::cmd::stage::run_reopen(&project_root, id),
            StageAction::Label { id, action, label } => {
                hlv::cmd::stage::run_label(&project_root, id, &action, &label)
            }
            StageAction::Meta {
                id,
                action,
                key,
                value,
            } => hlv::cmd::stage::run_meta(&project_root, id, &action, &key, value.as_deref()),
        },
    }
}
