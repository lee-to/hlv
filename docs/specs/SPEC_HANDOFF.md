# HANDOFF Service Specification

Status: Draft v1

Purpose: define an external agent orchestration service on top of `hlv mcp` without changing the HLV core.

Prerequisites:

- The developer MUST read the HLV documentation before implementing HANDOFF:
  - `docs/ARCH.md` — architecture, data model, project structure
  - `docs/WORKFLOW.md` — milestone/stage/task lifecycle and phase transitions
  - `docs/SPECS.md` — YAML/JSON schemas, file formats, project structure reference
  - `docs/MCP.md` — MCP server specification, transport modes, tool/resource reference
- HLV MCP tools and resources are self-describing via MCP `tools/list` and `resources/list`. Exact parameter signatures and payload schemas are available at runtime through MCP discovery.
- This specification does NOT duplicate HLV MCP schemas. It defines only the behavioral contracts, side effects, and invariants that are not visible from schema discovery alone.

Implementation freedom:

- The developer chooses the programming language, framework, and technology stack.
- The developer chooses which coding agents to use as workers (Claude Code, Codex, Cursor, custom agents, etc.).
- The developer chooses the transport and deployment model (single binary, microservices, serverless, etc.).
- This specification defines the **contract** (data model, state machines, invariants, APIs), not the implementation.
- Any implementation that satisfies the conformance checklist in section 20 is valid regardless of technology choices.

---

## 1. Problem Statement

HLV already serves as the source of truth for project structure:

- milestones, stages, tasks, and their statuses;
- task dependencies;
- project workflow status;
- labels/meta on milestones, stages, and tasks;
- MCP resources/tools for reading and managing this state.

This is sufficient for task management, but not sufficient for a full multi-agent orchestration system.

Problems `HANDOFF` must solve:

- multiple agents must be able to take tasks safely without double-claiming;
- each agent must operate in an isolated workspace;
- changes to shared files must trigger handoff/change propagation;
- the runtime state of agents must be observable and recoverable;
- the orchestrator must support retry, reconciliation, and recovery after restart;
- HLV state must remain canonical for project/stage/task state rather than being duplicated.

Important boundary:

- `HLV` does not become a runtime orchestrator.
- `HANDOFF` does not replace HLV as the project/task authority.
- `HANDOFF` uses `hlv mcp` as the authoritative backend for the task graph and lifecycle transitions.

Terminology note:

- `HLV MCP workspace mode` means one MCP server exposing multiple HLV projects through `hlv://projects/{id}/...` resources and `project_id`-scoped tools.
- `HANDOFF task workspace` means the isolated filesystem/worktree/sandbox in which an agent edits code for one claim/run.
- These are different concepts and MUST NOT be conflated in implementation docs or APIs.

---

## 2. Goals and Non-Goals

### 2.1 Goals

- Build an external orchestration service without changing `src/model`, `src/check`, `src/mcp`, or other HLV core layers.
- Use `hlv mcp` as the only official interface to HLV state.
- Support bounded-concurrency agent dispatch over ready tasks.
- Support durable task claims, agent registration, run attempts, retries, and reconciliation.
- Support workspace isolation and handoff between agents.
- Project meaningful orchestration information back into `milestones.yaml` through `meta`.
- Provide a complete contract that an AI can use to implement the system.

### 2.2 Non-Goals

- Changing the `project.yaml` or `milestones.yaml` schema.
- Adding new required HLV MCP tools to the HLV core.
- Replacing `hlv task start/done/block/...` with an internal HANDOFF state machine.
- A fully distributed scheduler with multiple active orchestrator writers.
- Mandating a specific UI, web dashboard, or database.

---

## 3. System Overview

### 3.1 Main Components

1. `HLV MCP Client`
   - Reads resources `hlv://workflow`, `hlv://tasks`, `hlv://milestones`, `hlv://plan`, `hlv://contracts`.
   - Calls tools `hlv_task_*`, `hlv_stage_*`, `hlv_milestone_*`, `hlv_check`, `hlv_workflow`.
   - In HLV MCP workspace mode, discovers projects through `hlv://projects` and scopes all resource/tool access by `project_id`.

2. `HANDOFF Orchestrator`
   - The only writer of orchestration state.
   - Selects ready tasks, assigns them to agents, and runs reconciliation and retries.

3. `Runtime Store`
   - Stores the agent registry, task claims, run attempts, live sessions, file claims, and event log.
   - Can be implemented using SQLite, an append-only journal, an embedded DB, or an equivalent durable store.

4. `Workspace Manager`
   - Creates and reuses a workspace/worktree/sandbox per task or per agent.
   - Validates paths and handles cleanup/reuse.

5. `Agent Runner`
   - Launches the coding agent.
   - Passes prompt/context.
   - Collects runtime events.

6. `Conflict Engine`
   - Tracks file claims, shared zones, and impact analysis.
   - Decides whether an agent may write to a file at a given moment.

7. `Projection Layer`
   - Writes summary orchestration state into `task.meta`, `stage.meta`, and `milestone.meta` via `hlv_*_meta`.

8. `Observability Surface`
   - Logs, event stream, status API, optional dashboard/TUI.

### 3.2 Responsibility Split

`HLV` is responsible for:

- the task graph and dependencies;
- stage/milestone/task lifecycle;
- human-readable project state;
- contracts, workflow, constraints, gates;
- machine-readable access through MCP.

`HANDOFF` is responsible for:

- which actor claimed a task;
- which agent/session/attempt is executing it;
- where the workspace is;
- whether it is safe to write a file;
- retries, backoff, reconciliation;
- propagating changes into dependent tasks and agent sessions.

### 3.3 Key Design Constraint

`meta` in HLV is only a summary projection, not a full runtime store.

Reasons:

- `meta` is typed as `HashMap<String, String>`;
- there are no CAS/lease/heartbeat semantics;
- multiple agents must not compete directly to write `milestones.yaml`;
- storing history, attempts, and an event stream in `meta` is awkward and expensive.

Consequences:

- All frequent runtime writes go to the `HANDOFF Runtime Store`.
- Only stable and useful summary state is projected into `HLV meta`.

---

## 4. HLV MCP Compatibility Contract

### 4.1 Required HLV Resources

Single-project mode:

`HANDOFF` must be able to read at minimum:

- `hlv://workflow`
- `hlv://tasks`
- `hlv://tasks/{n}` — where `{n}` is the `task_id` string (e.g. `hlv://tasks/TASK-002`)
- `hlv://stage/{n}` — where `{n}` is the stage number from `stage_N.md` filename (e.g. `hlv://stage/1`)
- `hlv://milestones`
- `hlv://plan`
- `hlv://contracts`
- `hlv://contracts/{id}`
- `hlv://constraints`

Optional but recommended:

- `hlv://project`
- `hlv://traceability`
- `hlv://artifacts`

Workspace mode:

- `HANDOFF` MUST also support `hlv://projects` for project discovery when connected to an HLV MCP server running in workspace mode.
- In workspace mode, every required single-project resource above MUST be treated as having a scoped equivalent under `hlv://projects/{id}/...`.
- Minimum workspace-scoped resources are therefore:
  - `hlv://projects`
  - `hlv://projects/{id}`
  - `hlv://projects/{id}/workflow`
  - `hlv://projects/{id}/tasks`
  - `hlv://projects/{id}/tasks/{n}`
  - `hlv://projects/{id}/stage/{n}`
  - `hlv://projects/{id}/milestones`
  - `hlv://projects/{id}/plan`
  - `hlv://projects/{id}/contracts`
  - `hlv://projects/{id}/contracts/{cid}`
  - `hlv://projects/{id}/constraints`
- `HANDOFF` MAY normalize both server modes into one internal abstraction, but it MUST preserve `project_id` as part of runtime identity when using workspace mode.

### 4.2 HLV Status Enums

HLV stage statuses (the full lifecycle, in order):

- `pending` — stage not yet started
- `verified` — stage plan reviewed/verified (optional step before implementation)
- `implementing` — work in progress
- `implemented` — all tasks done, awaiting validation
- `validating` — validation in progress
- `validated` — stage fully validated

HLV task statuses:

- `pending` — not started
- `in_progress` — work in progress
- `done` — completed
- `blocked` — manually blocked with a reason

Important behavioral notes:

- `hlv_task_start` has a side effect: if the stage is in `pending` or `verified`, it is automatically moved to `implementing`.
- `hlv_task_start` validates `depends_on` before starting — if any dependency task is not `done`, the call fails with an error. Therefore, `HANDOFF` MUST verify dependency satisfaction before calling `hlv_task_start`, or handle the rejection gracefully without releasing the claim.
- `hlv_task_start` is NOT idempotent — calling it on an `in_progress` task returns an error. Therefore, `HANDOFF` MUST call it exactly once per claim and MUST NOT call it on retries of the same logical execution chain.
- `hlv_task_done` requires the task to be in `in_progress` — calling it on a `pending` or `done` task returns an error.
- `hlv_task_block` requires a `reason` parameter (mandatory). It can block from `pending` or `in_progress`. `hlv_task_unblock` restores the previous status.
- `hlv_task_sync` creates `StageEntry` records in `milestones.yaml` for stages that exist as `stage_N.md` files but have no tracker yet. It also adds task trackers for new tasks found in stage plans. A "conflict" occurs when sync would remove a task tracker that is not in `pending` status (i.e., a task that was started, completed, or blocked). In `force` mode, conflicting tasks are removed regardless of status.

### 4.3 Required HLV Tools

`HANDOFF` must be able to call:

- `hlv_task_list`
- `hlv_task_sync`
- `hlv_task_start`
- `hlv_task_done`
- `hlv_task_block`
- `hlv_task_unblock`
- `hlv_task_meta`
- `hlv_task_label`
- `hlv_stage_meta`
- `hlv_stage_label`
- `hlv_milestone_meta`
- `hlv_check`
- `hlv_workflow`

Optional but recommended:

- `hlv_gate_run`
- `hlv_commit_msg`
- `hlv_milestone_label`

Workspace mode tool rule:

- Current HLV MCP tools support workspace mode by requiring a `project_id` parameter on tool calls.
- Therefore, when `HANDOFF` is connected to an HLV MCP server in workspace mode, every HLV tool invocation listed above MUST carry the target `project_id`.
- A `HANDOFF` implementation MAY wrap that parameter internally, but it MUST NOT discard it from runtime identity or reconciliation logic.

Note:

- `hlv://tasks` and `hlv_task_list` are not sufficient for full orchestration logic because they do not contain `depends_on`, `output`, or complete stage task context.
- To compute the ready queue and dependency eligibility, `HANDOFF v1` MUST use `hlv://stage/{n}` as the task-level source of truth.
- `hlv://stage/{n}` provides task context (`depends_on`, `contracts`, `output`) but does not define the full conflict policy. Therefore, write scopes MUST be determined by a separate HANDOFF policy layer, which MAY use `stage.output`, task labels, external config, and naming conventions.
- For conformance, that policy layer MUST emit a normalized `write_scope` model with deterministic precedence. Minimum required fields: `scope_kind`, `scope_value`, `mode`, `source`, `authoritative`.
- `scope_value` MUST be expressed relative to the canonical project root using normalized forward-slash paths and MUST NOT contain `..` traversal.
- Minimum supported `scope_kind` values are `exact_path`, `path_prefix`, and `glob`.
- Minimum supported `mode` values are `exclusive`, `shared-read`, and `shared-write`.
- Source precedence for conflicting declarations MUST be documented and deterministic. Recommended default precedence: explicit external HANDOFF config > manual stage/task overrides > task labels > naming conventions > `stage.output`.
- `HANDOFF` MUST NOT pretend that `hlv://stage/{n}` alone is sufficient for precise file-claiming if the actual scope policy is derived from other sources.
- `hlv://plan` MAY be used as a high-level overview of milestone/stage sequencing, but MUST NOT be treated as a sufficient source of task-level `depends_on`, `output`, or dispatch eligibility without checking against `hlv://stage/{n}`.
- `HANDOFF` MUST call `hlv_task_sync` at startup before the first scheduling pass.
- If `stage_*.md` may have changed outside the orchestrator, `HANDOFF` MUST call `hlv_task_sync` before the next scheduling pass; otherwise, `milestones.yaml` and `stage_*.md` are considered drifted and new dispatches must be paused.
- Current HLV MCP `resources/subscribe` tracks `milestones.yaml`, `project.yaml`, and `gates-policy.yaml` for a project, but not `human/milestones/*/stage_*.md`.
- In workspace mode the notification URIs become project-scoped (`hlv://projects/{id}/...`), but the `stage_*.md` limitation remains unchanged.
- Therefore, reliable drift detection for `stage_*.md` requires a separate file watcher/hash scan outside the current HLV MCP surface.
- If reliable drift detection for `stage_*.md` is unavailable, the safe default for `HANDOFF` is to call `hlv_task_sync` before every scheduling pass. For MVP, this is the recommended approach — the cost of `hlv_task_sync` is negligible for projects with fewer than 100 tasks and does not require optimization.
- The uniqueness check for dispatchable `task_id` values in the active milestone MUST run not only at startup, but also after every successful `hlv_task_sync` and after any drift event that could change stage/task composition.
- In workspace mode, that uniqueness check MUST run independently per `project_id`.
- If a new `task_id` ambiguity is detected after resync, `HANDOFF` MUST immediately stop new lifecycle-mutating dispatch decisions for the affected milestone, mark the milestone as non-conformant, and move the system into an operator-required degraded state until the data is corrected.

Scope note:

- The current HLV MCP task/contract surface works only relative to the `current` milestone of the addressed project.
- Therefore:
  - in single-project MCP mode, a single `HANDOFF` instance orchestrates exactly one active milestone at a time;
  - in workspace MCP mode, a single `HANDOFF` instance MAY orchestrate multiple projects, but still only one active milestone per project at a time.
- `history` milestones MAY be used for analytics and audit, but are not dispatch targets without a separate upstream change to the HLV contract.
- In current HLV, lifecycle tools `hlv_task_start`, `hlv_task_done`, `hlv_task_block`, `hlv_task_unblock`, and `hlv_task_meta` address a task only by plain `task_id` inside the `current` milestone of the addressed project.
- Therefore, `HANDOFF v1` MUST require `task_id` uniqueness within the active milestone of each addressed project for all dispatchable tasks.
- If two task trackers with the same `task_id` are found in different stages of one active milestone of one addressed project, `HANDOFF` MUST treat that project milestone as non-conformant, MUST NOT perform lifecycle-mutating operations for those tasks, and MUST move dispatch for that project into an operator-required error state until the milestone data is corrected.
- The current public HLV MCP does not provide a separate tool for directly mutating `stage.status`.
- `hlv_task_start` has an observable side effect: when a task is started for the first time, a stage in `pending` or `verified` is automatically moved to `implementing`.
- Therefore, `HANDOFF v1` is the authority for agent/runtime orchestration and task-level lifecycle transitions, and it indirectly initiates stage entry into `implementing` through `hlv_task_start`, but it is not the general authority for stage advancement.
- Stage-level status transitions (`implementing` -> `implemented`, `implemented` -> `validating`, etc.) remain the responsibility of an external workflow driver/operator on top of HLV until upstream adds an explicit stage lifecycle API.
- `HANDOFF` SHOULD project stage readiness through `stage.meta` and MUST NOT treat the runtime store as the source of truth for `stage.status`.

### 4.4 HLV MCP Connection

`HANDOFF` connects to `hlv mcp` using MCP protocol. Example configurations:

Single-project mode (stdio transport):

```json
{
  "hlv_mcp": {
    "transport": "stdio",
    "command": "hlv",
    "args": ["mcp"],
    "cwd": "/path/to/project"
  }
}
```

Single-project mode (SSE transport):

```json
{
  "hlv_mcp": {
    "transport": "sse",
    "url": "http://localhost:3000/mcp"
  }
}
```

Workspace mode:

```json
{
  "hlv_mcp": {
    "transport": "stdio",
    "command": "hlv",
    "args": ["mcp", "--workspace", "/path/to/workspace.yaml"]
  }
}
```

The exact config format is implementation-defined; these examples illustrate the required connection parameters.

### 4.5 Single-Writer Rule

Only the orchestrator process may write orchestration data back into HLV via `*_meta` and `hlv_task_*`.

Agents must not call these directly:

- `hlv_task_start`
- `hlv_task_done`
- `hlv_task_block`
- `hlv_task_unblock`
- `hlv_task_meta`
- `hlv_task_label`
- `hlv_stage_meta`
- `hlv_stage_label`
- `hlv_milestone_meta`
- `hlv_milestone_label`

Otherwise, there is a race over `milestones.yaml` and the single orchestration model breaks down.

### 4.6 HLV MCP Behavioral Notes

Exact payload schemas and tool parameter signatures are available at runtime through MCP discovery (`tools/list`, `resources/list`, `resources/read`). This section documents only behavioral contracts not visible from schema discovery.

#### Resource semantics

- `hlv://tasks` and `hlv_task_list` return the same structure — an array of task tracker views from `milestones.yaml`. This is runtime state, not the stage plan.
- `hlv://stage/{n}` returns the stage *plan* from `stage_N.md` — task definitions with `depends_on`, `output`, `contracts`. This is the planning source, not runtime state. For runtime task status, use `hlv://tasks`.
- `hlv://milestones` returns the full milestone state including `current` and `history`. The `current.stages` array contains `StageEntry` objects with embedded task trackers.
- `hlv://contracts/{id}` returns contract content in both markdown and YAML formats when both exist. For HANDOFF context assembly, the implementation MAY embed the full contract object or pass a reference with a content digest.
- Optional fields with empty values (`null`, `[]`, `{}`) may be omitted from responses.

#### Tool return conventions

All HLV MCP tools follow the MCP `CallToolResult` protocol:

- **Success**: `{"content": [{"type": "text", "text": "..."}], "isError": false}`
- **Error**: MCP error with code and message

Lifecycle tools (`hlv_task_start`, `hlv_task_done`, `hlv_task_block`, `hlv_task_unblock`) return plain text messages on success (e.g. `"Task 'TASK-001' started"`).

`hlv_task_sync` returns `"Tasks synced from stage plans"` on success. On conflict (non-force mode), it returns an MCP error with a descriptive message listing the conflicting tasks. The error message is human-readable text, not structured JSON.

#### Meta and label tools

- `hlv_task_meta`, `hlv_stage_meta`, `hlv_milestone_meta` accept: `action` (`set`|`delete`), `key`, `value` (required for `set`). `hlv_stage_meta` also requires `stage_id`.
- `hlv_task_label`, `hlv_stage_label`, `hlv_milestone_label` accept: `action` (`add`|`remove`), `label`. `hlv_stage_label` also requires `stage_id`.
- `meta` values are `HashMap<String, String>` — only string keys and string values. Complex state must be serialized to JSON string.

---

## 5. Core Domain Model

### 5.1 Entities

#### 5.1.1 HlvTaskRef

A stable reference to an HLV task.

Fields:

- `project_id`
- `milestone_id`
- `stage_id`
- `task_id`
- `task_status`
- `task_labels`
- `task_meta`

#### 5.1.2 Agent

A registered worker.

Fields:

- `agent_id`
- `agent_kind`
- `capabilities`
- `status`
- `last_heartbeat_at`
- `current_run_id`
- `workspace_root`

#### 5.1.3 TaskClaim

A runtime claim for a specific HLV task.

Fields:

- `claim_id`
- `task_ref`
- `agent_id`
- `active_run_id`
- `claimed_at`
- `lease_expires_at`
- `status`
- `claim_version`

#### 5.1.4 Workspace

An isolated working directory.

Fields:

- `workspace_id`
- `workspace_path`
- `workspace_kind`
- `task_ref`
- `agent_id`
- `created_at`
- `reused`

#### 5.1.5 RunAttempt

One attempt to execute a task by an agent.

Fields:

- `claim_id`
- `run_id`
- `task_ref`
- `agent_id`
- `attempt`
- `workspace_id`
- `status`
- `started_at`
- `finished_at`
- `error_code`
- `error_message`

#### 5.1.6 LiveSession

The state of a live agent session.

Fields:

- `session_id`
- `run_id`
- `thread_id`
- `turn_id`
- `pid`
- `last_event_at`
- `input_tokens`
- `output_tokens`
- `total_tokens`

#### 5.1.7 FileClaim

The right to modify a file or logical code zone.

Fields:

- `file_claim_id`
- `path_glob`
- `owner_run_id`
- `mode` (`exclusive` | `shared-read` | `shared-write`)
- `declared_at`
- `released_at`

#### 5.1.8 HandoffEvent

A normalized runtime event.

Fields:

- `event_id`
- `event_type`
- `timestamp`
- `task_ref`
- `claim_id`
- `run_id`
- `agent_id`
- `payload`

### 5.2 Stable Identifier Rules

- The canonical runtime identity of a task MUST be the composite `task_ref = (project_id, milestone_id, stage_id, task_id)`.
- Plain `task_id` SHOULD be treated as an HLV lifecycle lookup key inside the `current` milestone, not as a globally unique identifier for the entire system.
- `project_id` is mandatory in runtime state whenever `HANDOFF` is connected to HLV MCP workspace mode.
- Because of the current HLV API, HANDOFF runtime MAY store `stage_id`, but lifecycle writes in HLV v1 MUST only be performed for projects/milestones where `task_id` is unique within the active milestone.
- `claim_id` MUST be stable for one logical execution chain of a task from the first successful claim to final release.
- `run_id` MUST be globally unique and identify exactly one specific attempt within `claim_id`.
- `workspace_id` MUST be stable within one task lifecycle if the workspace is reused.
- `agent_id` MUST be stable across heartbeats from the same agent.

---

## 6. Meta Projection Contract

### 6.1 Projection Purpose

`meta` is used as a visible projection of HANDOFF state for:

- HLV MCP clients;
- dashboard/TUI;
- manual diagnostics in `milestones.yaml`;
- degraded restart diagnostics and last-known summary if the runtime store is unavailable or partially corrupted.

Important:

- `meta` MUST NOT be considered a sufficient source for full runtime recovery.
- If the runtime store is lost or damaged, `meta` MAY help the operator understand the last known state, but it MUST NOT reconstruct active claims, live leases, pending retries, or pending HLV lifecycle intents on its own.
- Resuming/replaying orchestration after runtime-store loss requires either an intact event/runtime store or explicit operator reconciliation.

### 6.2 Recommended Task Meta Keys

`HANDOFF` SHOULD use namespaced keys:

- `handoff.assignee`
- `handoff.claim_id`
- `handoff.run_id`
- `handoff.session_id`
- `handoff.workspace`
- `handoff.status`
- `handoff.attempt`
- `handoff.claimed_at`
- `handoff.last_heartbeat_at`
- `handoff.last_result`
- `handoff.retry_count`
- `handoff.conflict_state`
- `handoff.blocked_by`

### 6.3 Recommended Stage Meta Keys

- `handoff.stage_coordinator`
- `handoff.parallelism`
- `handoff.ready_tasks`
- `handoff.running_tasks`
- `handoff.blocked_tasks`
- `handoff.last_reconcile_at`

### 6.4 Recommended Milestone Meta Keys

- `handoff.orchestrator_id`
- `handoff.orchestrator_version`
- `handoff.runtime_store`
- `handoff.active_agents`
- `handoff.active_runs`
- `handoff.dashboard_url`
- `handoff.policy`

### 6.5 Projection Rules

- Projection MUST be performed only by the orchestrator.
- Projection SHOULD happen only on meaningful state transitions, not on every heartbeat.
- Projection MUST be idempotent.
- Projection SHOULD use compact string values suitable for manual reading.

Atomicity note:

- Current HLV MCP updates `meta` one key per call.
- Therefore, `HANDOFF` MUST NOT rely on multi-key atomic updates in `task.meta`, `stage.meta`, or `milestone.meta`.
- If consistent reads across multiple fields are required, `HANDOFF` SHOULD use one aggregated key, such as `handoff.state`, with a stringified JSON payload.
- If multiple keys are used, `HANDOFF` MUST add `handoff.projection_version` and treat the write as complete only after the version marker is updated.
- Readers SHOULD ignore a partially updated projection without a valid final version marker.

---

## 7. Orchestration State Machine

### 7.1 Task Orchestration States

The internal `HANDOFF` state is not the same as HLV `TaskStatus`.

`HANDOFF` task states:

1. `Unclaimed`
2. `ClaimPending`
3. `Claimed`
4. `Running`
5. `WaitingHandoff`
6. `AwaitingLifecycleCommit`
7. `Blocked`
8. `RetryQueued`
9. `Completed`
10. `Released`
11. `FailedTerminal`

Canonical naming rule:

- TitleCase names in this section define the logical orchestration states.
- Persisted/API enum values MUST use stable `snake_case`.
- The canonical wire/storage spellings are:
  - `Unclaimed` -> `unclaimed`
  - `ClaimPending` -> `claim_pending`
  - `Claimed` -> `claimed`
  - `Running` -> `running`
  - `WaitingHandoff` -> `waiting_handoff`
  - `AwaitingLifecycleCommit` -> `awaiting_lifecycle_commit`
  - `Blocked` -> `blocked`
  - `RetryQueued` -> `retry_queued`
  - `Completed` -> `completed`
  - `Released` -> `released`
  - `FailedTerminal` -> `failed_terminal`
- Implementations MUST NOT invent parallel spellings such as `awaiting_hlv_commit` for the same logical state. If a storage column or API field uses a state enum, it MUST use the canonical `snake_case` spelling from this list.

Mapping to HLV:

- on successful claim, the orchestrator calls `hlv_task_start`;
- on successful completion, the orchestrator calls `hlv_task_done`;
- on external blocking, the orchestrator may call `hlv_task_block`;
- terminal failure without retry MUST be represented in HLV via `hlv_task_block`, because HLV has no separate `failed` status;
- intermediate runtime states (`ClaimPending`, `WaitingHandoff`, `RetryQueued`) are stored only in HANDOFF and projected via `meta`.
- `ClaimPending` is the required state for a claim that exists in the runtime store but whose initial `hlv_task_start` outcome is not yet confirmed and therefore must not launch a worker yet.
- `AwaitingLifecycleCommit` means the worker already finished the run, but the required HLV lifecycle write (`hlv_task_done` or terminal `hlv_task_block`) has not yet been committed and must be replayed by the reconciliation loop without relaunching the worker.

Important retry rule:

- After the first successful `hlv_task_start`, the HLV task is usually in `in_progress`.
- Therefore, retrying the same logical execution chain MUST be allowed on top of HLV `in_progress` if the task is not yet completed and HANDOFF runtime state considers it retryable.
- `RetryQueued` does not require moving the HLV task back to `pending`; it is a HANDOFF runtime state, not an HLV lifecycle transition.
- If an HLV lifecycle write fails, the task does NOT become logically `Completed` or `Blocked` based on the runtime store alone; until replay succeeds, it remains a recoverable runtime entry with projection/lifecycle drift.

Required state mapping:

- `task_claims.status` is claim lifecycle, not run lifecycle:
  - `active` = claim currently owns the task and may have a live run, pending retry, or pending lifecycle intent
  - `released` = claim finished and no longer owns the task
  - `expired` = claim lost ownership due to lease expiry/reconciliation
  - `superseded` = claim was replaced by an operator-controlled recovery flow
- `run_attempts.status` is attempt lifecycle and MUST use:
  - `preparing`
  - `running`
  - `waiting_handoff`
  - `awaiting_lifecycle_commit`
  - `retry_queued`
  - `succeeded`
  - `failed`
  - `timed_out`
  - `stalled`
  - `cancelled`
- API responses that expose a task-level next state, such as `handoff_fail.next_state`, MUST use the canonical task-state enum:
  - `retry_queued`
  - `blocked`
  - `failed_terminal`
- A completed worker whose promotion or HLV lifecycle commit is still unresolved MUST keep:
  - `task_claims.status = active`
  - `run_attempts.status = awaiting_lifecycle_commit`
  - task projection state = `awaiting_lifecycle_commit` or a stricter subtype documented by the implementation

Transition authority rule:

- `task_claims.status`, `run_attempts.status`, and projected task state describe different layers and MUST NOT be collapsed into one shared enum.
- The implementation MUST define one transition table that lists, for every orchestrator action, the old/new values for:
  - logical task orchestration state
  - `task_claims.status`
  - `run_attempts.status`
  - HLV task status
  - pending intent status when applicable
- That transition table MAY live in implementation docs or code comments, but it is required for conformance.

Reference transition table (implementations MAY extend but MUST NOT contradict):

```
Action                  | Task State (before → after)           | claim.status | run.status                    | HLV task      | Pending intent
------------------------|---------------------------------------|--------------|-------------------------------|---------------|---------------
new_claim               | unclaimed → claim_pending             | active       | preparing                     | pending       | task_start
hlv_task_start OK       | claim_pending → claimed               | active       | preparing                     | in_progress   | (resolved)
hlv_task_start ambiguous| claim_pending → claim_pending         | active       | preparing                     | unknown       | task_start
hlv_task_start fail     | claim_pending → released              | released     | cancelled                     | pending       | (cancelled)
launch_worker           | claimed → running                     | active       | running                       | in_progress   | —
worker_succeeded        | running → awaiting_lifecycle_commit   | active       | awaiting_lifecycle_commit     | in_progress   | promotion
promotion OK            | awaiting_lifecycle_commit → completed | active       | awaiting_lifecycle_commit     | in_progress   | task_done
hlv_task_done OK        | completed → released                  | released     | succeeded                     | done          | (resolved)
hlv_task_done fail      | completed → awaiting_lifecycle_commit | active       | awaiting_lifecycle_commit     | in_progress   | task_done
worker_failed(retry)    | running → retry_queued                | active       | failed                        | in_progress   | —
retry_timer_fired       | retry_queued → running                | active       | running (new run_id)          | in_progress   | —
worker_failed(terminal) | running → failed_terminal             | active       | failed                        | in_progress   | task_block
hlv_task_block OK       | failed_terminal → released            | released     | failed                        | blocked       | (resolved)
lease_expired           | running → retry_queued                | active       | stalled                       | in_progress   | —
conflict_detected       | running → waiting_handoff             | active       | waiting_handoff               | in_progress   | —
handoff_resolved        | waiting_handoff → running             | active       | running                       | in_progress   | —
```

### 7.2 Run Attempt Lifecycle

Run attempt phases:

1. `SelectingTask`
2. `ClaimingTask`
3. `PreparingWorkspace`
4. `BuildingContext`
5. `LaunchingAgent`
6. `InitializingSession`
7. `StreamingRun`
8. `CheckingConflicts`
9. `WaitingForHandoff`
10. `Finishing`
11. `Succeeded`
12. `Failed`
13. `TimedOut`
14. `Stalled`
15. `CanceledByReconciliation`

### 7.3 Transition Triggers

- `Poll Tick`
- `Ready Task Discovered`
- `Claim Acquired`
- `Agent Event`
- `Lease Expired`
- `Retry Timer Fired`
- `Conflict Detected`
- `Dependency Changed`
- `Task No Longer Eligible`
- `Orchestrator Restart`

### 7.4 Idempotency Rules

- One active task claim may have only one `active_run_id` at any given time, but MAY have multiple sequential historical `run_id` values across retries.
- `hlv_task_start` MUST be called no more than once for a given claim.
- `hlv_task_done` MUST be called only for an active successful claim.
- Re-projecting into `meta` must not change the meaning of the state.

---

## 8. Scheduling, Claiming, and Reconciliation

### 8.1 Ready Task Discovery

Source of truth for eligibility:

- synchronized HLV task trackers and stage plans;
- HLV task dependency graph;
- HLV task status;
- orchestration policy HANDOFF;
- optional labels/meta filters.

A task is dispatch-eligible only if:

- its HLV status is `pending`, or `in_progress` under an active HANDOFF retry/continuation policy;
- all `depends_on` are satisfied (a dependency is satisfied when the referenced task has HLV status `done`; dependencies MAY reference tasks in other stages — cross-stage dependencies are supported by HLV);
- there is no active claim on it for a new dispatch;
- a `current` milestone exists; milestones from `history` do not participate in dispatch;
- the required `hlv_task_sync` was executed before computing the ready queue, or the orchestrator proved there is no drift between the `milestones.yaml` tracker and `stage_*.md`;
- the stage is in a schedulable HLV status. For a new dispatch, this SHOULD mean `pending`, `verified`, or `implementing`;
- a stage in `implemented`, `validating`, or `validated` SHOULD NOT receive new dispatches without explicit operator policy/reopen semantics outside the base HLV workflow;
- global and per-agent concurrency slots are available.

More precise rule:

- A new task that has never been started before must begin only from HLV `pending`.
- A retry dispatch is not a new claim from the general ready queue; it is a continuation of an existing `claim_id` selected from the orchestrator's retry queue.
- A retry or continuation of the same task MAY start from HLV `in_progress` if:
  - there is exactly one active claim for that logical execution chain;
  - the new `run_id` is created as the next attempt inside that `claim_id`;
  - the run has not previously been completed via `hlv_task_done`;
  - the task has not been moved to HLV `blocked`;
  - the runtime store contains a retryable terminal outcome from the previous attempt.

### 8.2 Claim Protocol

Recommended order for a new dispatch:

1. The orchestrator selects a ready task.
2. Before claiming, it ensures `hlv_task_sync` is current for that scheduling epoch.
3. It creates a `TaskClaim` in the runtime store.
4. It creates the first `RunAttempt` and records it as `active_run_id`.
5. If the claim is successfully established, it calls `hlv_task_start`.
6. It writes a summary into `task.meta`.
7. It launches the worker.

Recommended order for a retry dispatch:

1. The orchestrator selects a due retry for an existing active `claim_id`.
2. It creates a new `RunAttempt` with a new `run_id` and incremented `attempt`.
3. It updates `task_claims.active_run_id`.
4. It does NOT call `hlv_task_start` if the HLV task is already in `in_progress`.
5. It updates the summary in `task.meta`.
6. It relaunches the worker.

If `hlv_task_start` fails:

- if the failure is deterministic and known-not-applied (for example: validation error, task not found, task already terminal), the claim MUST be released;
- if the failure is transport-level or otherwise outcome-ambiguous, the claim MUST NOT be released immediately;
- an outcome-ambiguous `hlv_task_start` failure MUST create a durable pending lifecycle intent for `task_start`, keep the claim in `ClaimPending` or an equivalent recoverable pre-launch state, and defer worker launch until reconciliation confirms whether HLV observed the start;
- reconciliation for an ambiguous start MUST either:
  - confirm that HLV already transitioned the task into a compatible started state and then continue with the existing claim; or
  - replay `hlv_task_start` idempotently for the same claim; or
  - mark the claim failed/released only after confirming that the start did not take effect;
- the projection MUST reflect `start_pending_confirmation`, `start_failed`, or an equivalent distinct state rather than collapsing all start errors into an immediate release.

If `hlv_task_sync` fails due to drift/conflict between the tracker and the stage plan:

- the orchestrator MUST pause only new dispatches for the affected milestone or scheduling epoch;
- already active runs/claims MUST NOT be treated as invalid automatically just because of a sync conflict;
- the orchestrator MUST mark the milestone/task projection as `sync_conflict` or an equivalent drift state;
- the orchestrator MUST enter reconciliation mode: either wait for operator resolution or execute a policy-driven `hlv_task_sync --force` if that policy is explicitly allowed;
- until the conflict is resolved, `HANDOFF` MUST NOT create new claims for tasks whose eligibility depends on the unsynchronized stage tracker.

If retry is scheduled after a worker failure:

- the orchestrator MUST NOT call `hlv_task_start` again if the HLV task is already in `in_progress` because of a previous successful claim for the same logical task;
- subsequent retry attempts reuse the active HLV started state, keep the same `claim_id`, and create only a new HANDOFF `run_id`/attempt.

### 8.3 Concurrency Control

`HANDOFF` SHOULD support:

- global concurrency;
- per-stage concurrency;
- per-agent concurrency;
- optional capability-based routing.

### 8.4 Retry and Backoff

Failure-driven retry:

- exponential backoff;
- capped max delay;
- an upper bound on attempts.

Continuation/handoff retry:

- a short fixed delay;
- used to re-check dependent/shared tasks after propagation.

### 8.5 Reconciliation

On every tick, the orchestrator MUST:

1. Check live runs for stall/timeout.
2. Compare runtime claims against HLV task state.
3. Release claims for completed/invalid tasks.
4. Rebuild the ready queue.
5. Re-project summary state when drift is detected.

### 8.6 Restart Recovery

After restart, the orchestrator MUST be able to:

- load the runtime store;
- re-read `hlv://tasks`, `hlv://milestones`, and `hlv://workflow`;
- reconcile active claims with actual HLV state;
- cancel or requeue orphaned runs;
- restore summary projection when needed.

Recovery boundary:

- `meta` MAY be used only as a last-known projection hint for drift detection and operator diagnostics.
- `HANDOFF` MUST NOT recreate an active claim, lease, retry timer, or pending lifecycle replay solely from data in `task.meta`, `stage.meta`, or `milestone.meta`.
- If the runtime store is lost and replay intents are unavailable, the orchestrator MUST enter a safe degraded mode: forbid new dispatches, re-read HLV state, mark the runtime state as requiring operator reconciliation, and wait for an explicit operator decision.

---

## 9. Workspace Management

### 9.1 Workspace Model

Implementation may choose:

- per-task workspace;
- per-agent workspace;
- per-stage workspace;
- git worktree per task.

Recommended default:

- a separate workspace/worktree per task claim.

### 9.2 Safety Invariants

- workspace paths MUST be under the configured workspace root;
- the agent cwd MUST stay inside the assigned workspace;
- cleanup MUST not touch other workspaces;
- workspace naming MUST use sanitized milestone/stage/task identifiers.

### 9.3 Reuse Rules

- a workspace MAY be reused across retry attempts of the same task;
- a workspace SHOULD not be reused across different tasks without explicit policy;
- shared dependency artifacts SHOULD be passed through a handoff event or commit/patch boundary rather than through a shared writable root.

### 9.4 Promotion Back to Canonical Project State

An isolated workspace is not, by itself, authoritative project state.

Therefore:

- the implementation MUST define an explicit promotion/integration step from the task workspace back into the canonical repo/root;
- promotion MAY be implemented through a merge from a git worktree, apply-patch, controlled file sync, or an equivalent deterministic mechanism;
- regardless of transport, promotion MUST follow one deterministic protocol:
  1. capture a `base_revision` or equivalent canonical-tree snapshot identifier before the worker starts modifying the workspace;
  2. build a promotion artifact against that base (`git diff`, patch, file manifest, or equivalent deterministic delta);
  3. serialize promotion into the canonical tree so that only one promotion is committed at a time;
  4. verify that the canonical tree still matches the expected `base_revision`, or perform an explicitly documented deterministic rebase/reapply step before applying the artifact;
  5. if the artifact no longer applies cleanly, persist a `promotion` pending intent and move the run into an operator-visible integration-conflict state without calling `hlv_task_done`;
  6. compute the authoritative changed file set from the applied promotion artifact or from the resulting canonical-tree diff after successful promotion;
  7. persist the promotion outcome with enough data to replay or audit it (`base_revision`, artifact digest/reference, resulting canonical revision);
- the authoritative changed file set for propagation and downstream scheduling MUST be computed after this promotion step, or from the same promotion artifact that updates the canonical tree;
- `hlv_task_done` MUST NOT be called before changes are successfully promoted/integrated into canonical project state;
- if the worker finishes successfully but promotion fails, the run MUST transition into a recoverable non-completed state (`AwaitingLifecycleCommit` or an equivalent integration-failed state) without relaunching the completed worker before reconciliation/operator resolution.

Recommended MVP promotion using git worktrees:

```text
# 1. Create workspace before worker launch
base_rev = git rev-parse HEAD
git worktree add {workspace_root}/{claim_id} -b handoff/{task_id}-{claim_id} {base_rev}

# 2. Worker operates inside the worktree
# (agent cwd = {workspace_root}/{claim_id})

# 3. After worker success — promote
cd {workspace_root}/{claim_id}
git add -A && git commit -m "handoff: {task_id} attempt {attempt}"

# 4. Integrate into canonical tree
cd {project_root}
current_rev = git rev-parse HEAD
if current_rev == base_rev:
    git merge --ff-only handoff/{task_id}-{claim_id}
else:
    git rebase --onto HEAD {base_rev} handoff/{task_id}-{claim_id}
    # if rebase fails → persist promotion intent, do NOT call hlv_task_done

# 5. Compute changed files from the promotion
changed_files = git diff --name-only {base_rev}..HEAD

# 6. Cleanup
git worktree remove {workspace_root}/{claim_id}
git branch -d handoff/{task_id}-{claim_id}
```

This is a recommended default. Implementations MAY use alternative mechanisms (patch files, rsync, etc.) as long as the promotion protocol invariants above are satisfied.

---

## 10. Conflict Detection and Change Propagation

### 10.1 Problem

HLV understands task dependencies, but not runtime file conflicts.

Therefore, `HANDOFF` MUST implement a separate conflict engine.

### 10.2 Conflict Sources

- two agents want to write the same file;
- one agent changes a shared type/API that another task depends on;
- generated files overlap across tasks;
- the actual diff violates the declared task scope.

### 10.3 Conflict Protocol

Before writing a file, the worker MUST:

1. declare file intent;
2. ask the `Conflict Engine` for a grant;
3. receive `allow`, `allow_with_warning`, or `deny`;
4. on `deny`, move into `WaitingHandoff` or `Blocked`.

Conformance rule:

- If declared write scopes are known before the worker is launched, the orchestrator MUST reserve them before `LaunchingAgent` and reflect that reservation in `file_claims` or an equivalent runtime mechanism.
- `handoff_check`/runtime file intent MUST be used for dynamic scope escalation, shared-scope confirmation, or attempts to write outside the pre-reserved scope.
- `handoff_check` MUST NOT be the only protection mechanism if the scope policy can compute a conflict-prone write area ahead of time.

Normalized write-scope contract:

- `write_scopes` returned by dispatch and stored in runtime state MUST be an array of objects with:
  - `scope_kind`: `exact_path` | `path_prefix` | `glob`
  - `scope_value`: normalized project-relative path expression
  - `mode`: `exclusive` | `shared-read` | `shared-write`
  - `source`: `stage_output` | `task_label` | `naming_rule` | `external_policy` | `manual_override`
  - `authoritative`: boolean
- Path matching MUST be evaluated against canonical project-relative paths:
  - `exact_path` matches exactly one file;
  - `path_prefix` matches the named directory/file prefix and all descendants;
  - `glob` matches using one documented glob engine for the whole implementation.
- Overlap decisions for two active claims owned by different `claim_id` values MUST be deterministic:
  - `exclusive` overlapping any write-capable scope (`exclusive` or `shared-write`) = `deny`;
  - `shared-write` overlapping `shared-write` = `allow_with_warning` only if both scopes are marked shared-writable by policy for that path;
  - `shared-read` does not grant write permission by itself and MUST NOT satisfy a write request.
- If multiple scope declarations cover the same path for one task, the implementation MUST choose the most specific scope first. Recommended specificity order: `exact_path` > `glob` > `path_prefix`. Ties MUST be broken by the source precedence defined in section 4.3.
- If no predeclared scope covers a requested write path, `handoff_check` MUST treat that request as dynamic scope escalation and either create an explicit additional scope grant or deny it.

Normalized `handoff_check` contract:

- Input `paths` MUST be an array of normalized project-relative paths.
- Input `mode` MUST use the same enum as the stored write scopes.
- Output `conflicts` MUST be a structured array. Minimum fields per item:
  - `claim_id`
  - `run_id`
  - `scope_kind`
  - `scope_value`
  - `mode`
  - `reason`
- Output `required_handoff` MUST be a structured array of impacted task references or runs, not a free-form string.

### 10.4 Change Propagation

After a run completes, the orchestrator SHOULD:

1. compute the authoritative changed file set;
2. match those files against other active claims;
3. determine impacted tasks;
4. emit `handoff_required` or `context_refresh_required`;
5. requeue dependent tasks or move them to `Blocked` if necessary.

Authoritative source rule:

- the authoritative `changed file set` MUST be computed by the orchestrator from a workspace diff, VCS/worktree diff, filesystem snapshot diff, or an equivalent server-side mechanism;
- `changed_files` reported by the worker SHOULD be treated only as an advisory hint to speed up analysis;
- if self-reported `changed_files` differs from the server-side diff, the orchestrator MUST trust the server-side diff, log a mismatch event, and MAY mark the run as suspicious according to policy.

### 10.5 Minimal Viable Strategy

If there is no precise code ownership map, an incremental strategy is acceptable:

1. path-prefix ownership;
2. explicit shared-file denylist;
3. task label based routing;
4. semantic impact analysis later.

---

## 11. Agent Runner Protocol

### 11.1 Design Principles

`HANDOFF` SHOULD follow these architectural principles:

- long-running orchestrator process (not one-shot);
- isolated workspace per task claim;
- explicit session/run model with durable identity;
- structured runtime events for observability;
- retry/reconciliation loops for crash recovery.

### 11.2 Launch Contract

The runner MUST accept:

- `task_ref`
- `run_id`
- `attempt`
- `workspace_path`
- `prompt/context payload`
- approval/sandbox policy

The runner MUST emit:

- `session_started`
- `progress`
- `file_intent_declared`
- `conflict_detected`
- `handoff_requested`
- `turn_completed`
- `turn_failed`
- `run_succeeded`
- `run_failed`

Normalized `context_bundle` contract:

Reference JSON example:

```json
{
  "schema_version": "1.0",
  "task_ref": {
    "project_id": "my-project",
    "milestone_id": "001-order-create",
    "stage_id": 1,
    "task_id": "TASK-002"
  },
  "dispatch_kind": "new",
  "attempt": 1,
  "workspace": {
    "workspace_id": "ws-abc123",
    "workspace_path": "/workspaces/claim-xyz/",
    "workspace_kind": "git_worktree"
  },
  "write_scopes": [
    {
      "scope_kind": "path_prefix",
      "scope_value": "src/features/order_create/",
      "mode": "exclusive",
      "source": "stage_output",
      "authoritative": true
    }
  ],
  "base_revision": "a1b2c3d4e5f6",
  "hlv_snapshot": {
    "task_status": "in_progress",
    "stage_status": "implementing",
    "workflow": {
      "phase": 4,
      "phase_name": "Implement",
      "next_actions": ["Implementation in progress"]
    }
  },
  "task": {
    "name": "Create handler",
    "labels": ["api"],
    "meta": {},
    "output": ["src/features/order_create/"],
    "depends_on": ["TASK-001"],
    "contracts": ["order.create"]
  },
  "dependencies": [
    {
      "task_id": "TASK-001",
      "stage_id": 1,
      "status": "done",
      "changed_files": ["src/domain/order.rs", "src/domain/mod.rs"],
      "summary": "Domain types implemented"
    }
  ],
  "contracts": [
    {
      "id": "order.create",
      "version": "1.0",
      "intent": "Create a new order",
      "content_digest": "sha256:abcdef..."
    }
  ],
  "constraints": [
    {
      "id": "security",
      "rules": [
        {
          "id": "SEC-001",
          "severity": "critical",
          "statement": "All API endpoints must require authentication"
        }
      ]
    }
  ],
  "handoff": {
    "upstream_notes": [],
    "changed_files": ["src/domain/order.rs", "src/domain/mod.rs"],
    "unresolved_warnings": []
  },
  "policy": {
    "approval_mode": "auto",
    "timeout_seconds": 1800,
    "human_input": "never"
  }
}
```

- `context_bundle` MUST be a structured object, not an opaque prompt blob.
- Minimum required top-level fields:
  - `schema_version`
  - `task_ref`
  - `dispatch_kind`
  - `attempt`
  - `workspace`
  - `write_scopes`
  - `base_revision`
  - `hlv_snapshot`
  - `task`
  - `dependencies`
  - `contracts`
  - `constraints`
  - `handoff`
  - `policy`
- Minimum field semantics:
  - `schema_version`: version of the HANDOFF runner payload contract
  - `task_ref`: `{ project_id, milestone_id, stage_id, task_id }`
  - `dispatch_kind`: `new` | `retry`
  - `workspace`: `{ workspace_id, workspace_path, workspace_kind }`
  - `write_scopes`: normalized array from section 10.3
  - `base_revision`: canonical-tree snapshot/revision captured before workspace mutation
  - `hlv_snapshot`: minimally `{ task_status, stage_status, workflow }`
  - `task`: minimally `{ name, description?, labels, meta, output, depends_on }`
  - `dependencies`: array of upstream task summaries with enough data to understand completed prerequisites and pending handoffs
  - `contracts`: array of contract references or embedded normalized contract objects required for implementation
  - `constraints`: array of applicable project/stage/task constraints
  - `handoff`: structured upstream handoff notes, changed files, and unresolved warnings
  - `policy`: execution policy bundle including approval/sandbox mode, timeout policy, and human-input policy
- Implementations MAY add fields, but MUST NOT omit the required fields above.

Normalized runner event envelope:

- Every runner-emitted event MUST include:
  - `schema_version`
  - `event_id`
  - `event_type`
  - `occurred_at`
  - `run_id`
  - `attempt`
  - `task_ref`
  - `sequence`
  - `payload`
- `event_id` MUST be unique per emitted event.
- `sequence` MUST be monotonically increasing within one `run_id`.
- The orchestrator MUST treat duplicate `(run_id, sequence)` events as idempotent re-delivery.
- Event ordering guarantee:
  - order is required only within one `run_id`
  - cross-run global ordering is not required
- Minimum payload requirements by event type:
  - `session_started`: `session_id`, `thread_id?`, `pid?`
  - `progress`: `message`, `progress_kind?`, `changed_paths_hint?`
  - `file_intent_declared`: `paths`, `mode`
  - `conflict_detected`: `paths`, `decision`, `conflicts`
  - `handoff_requested`: `required_handoff`, `reason`
  - `turn_completed`: `turn_id?`, `summary?`
  - `turn_failed`: `turn_id?`, `error_code`, `error_message`
  - `run_succeeded`: `summary`, `changed_files_hint?`, `artifacts?`
  - `run_failed`: `failure_kind`, `error_code`, `error_message`, `retryable`

Reference runner event examples:

```json
{
  "schema_version": "1.0",
  "event_id": "evt-20260309-001",
  "event_type": "session_started",
  "occurred_at": "2026-03-09T12:00:01Z",
  "run_id": "run-20260309-001",
  "attempt": 1,
  "task_ref": {
    "project_id": "my-project",
    "milestone_id": "001-order-create",
    "stage_id": 1,
    "task_id": "TASK-002"
  },
  "sequence": 1,
  "payload": {
    "session_id": "sess-abc123",
    "pid": 12345
  }
}
```

```json
{
  "schema_version": "1.0",
  "event_id": "evt-20260309-007",
  "event_type": "run_succeeded",
  "occurred_at": "2026-03-09T12:28:15Z",
  "run_id": "run-20260309-001",
  "attempt": 1,
  "task_ref": {
    "project_id": "my-project",
    "milestone_id": "001-order-create",
    "stage_id": 1,
    "task_id": "TASK-002"
  },
  "sequence": 7,
  "payload": {
    "summary": "Implemented order.create handler with validation",
    "changed_files_hint": [
      "src/features/order_create/handler.rs",
      "src/features/order_create/mod.rs"
    ],
    "artifacts": []
  }
}
```

```json
{
  "schema_version": "1.0",
  "event_id": "evt-20260309-005",
  "event_type": "run_failed",
  "occurred_at": "2026-03-09T12:15:30Z",
  "run_id": "run-20260309-003",
  "attempt": 2,
  "task_ref": {
    "project_id": "my-project",
    "milestone_id": "001-order-create",
    "stage_id": 1,
    "task_id": "TASK-004"
  },
  "sequence": 5,
  "payload": {
    "failure_kind": "timeout",
    "error_code": "AGENT_TIMEOUT",
    "error_message": "Agent did not complete within 1800s",
    "retryable": true
  }
}
```

Runner delivery rules:

- The runner/orchestrator boundary MUST document whether events are push, pull, or streamed, but the envelope semantics above are mandatory regardless of transport.
- The orchestrator MUST durably persist terminal events before acknowledging terminal completion to the caller when transport supports acknowledgement.
- If the transport does not support acknowledgements, the orchestrator MUST still deduplicate by `event_id` or `(run_id, sequence)`.

Recommended MVP runner transport:

- For subprocess-based agents (Claude Code, Codex CLI, etc.): the runner wrapper writes one JSON-line per event to `stdout` or to a dedicated event file (`{workspace}/.handoff/events.jsonl`). The orchestrator reads events by tailing the stream or polling the file.
- For HTTP-based agents: the runner posts events to the orchestrator's `POST /api/v1/events/ingest` endpoint or equivalent.
- The MVP implementation MUST choose one transport and document it. The simplest viable default is `stdout` JSON-lines from a subprocess wrapper.

### 11.2.1 Reference Prompt Rendering

`context_bundle` is a structured object for the orchestrator. The agent runner MUST transform it into a prompt suitable for the target coding agent. The rendering format is implementation-defined, but the following reference example illustrates a recommended approach for Claude Code:

```text
# Task Assignment

You are implementing task TASK-002 (Create handler) in project my-project,
milestone 001-order-create, stage 1.

## Workspace

Your working directory is: /workspaces/claim-xyz/
You MUST write only within: src/features/order_create/ (exclusive)
Base revision: a1b2c3d4e5f6

## Task Description

Create the order.create handler with input validation.

### Dependencies (completed)

- TASK-001 (done): Domain types implemented
  Changed files: src/domain/order.rs, src/domain/mod.rs

### Contracts

**order.create** (v1.0): Create a new order
[Full contract content or reference here]

### Constraints

- [CRITICAL] SEC-001: All API endpoints must require authentication

## Instructions

1. Implement the task according to the contract specification above.
2. Stay within the declared write scope.
3. Do not modify files outside src/features/order_create/.
4. When done, report changed files and a brief summary.
```

Key rendering rules:

- The prompt MUST include: task description, write boundaries, dependency context, and applicable contracts.
- The prompt SHOULD include: constraints, upstream changed files, and workspace path.
- The prompt MUST NOT include: internal orchestration identifiers (claim_id, run_id, lease_expires_at) that are meaningless to the coding agent.
- The prompt format MAY vary by `agent_kind` (e.g. Claude Code vs Codex vs custom agents).
- The implementation MUST document its prompt template so that operators can audit and customize it.

### 11.3 Context Assembly

The minimal context bundle SHOULD include:

- task description from the stage plan;
- task dependencies;
- relevant contracts;
- HLV workflow/status;
- constraints and labels;
- handoff summary from upstream tasks;
- promotion base revision or equivalent canonical-tree snapshot identifier;
- workspace path and write boundaries.

HLV MCP source mapping for `context_bundle` fields:

```
context_bundle field     | HLV MCP source                                         | Notes
-------------------------|--------------------------------------------------------|-------------------------------
task_ref.project_id      | hlv://projects (workspace) or config (single)          | stable, cached at startup
task_ref.milestone_id    | hlv://milestones → current.id                          | cached per scheduling epoch
task_ref.stage_id        | hlv://stage/{n} → id                                   | from stage plan
task_ref.task_id         | hlv://stage/{n} → tasks[].id                           | from stage plan
task                     | hlv://stage/{n} → find task by id                      | name, depends_on, output, contracts
hlv_snapshot.task_status | hlv://tasks or hlv_task_list                            | current TaskStatus
hlv_snapshot.stage_status| hlv://milestones → current.stages[n].status            | current StageStatus
hlv_snapshot.workflow    | hlv://workflow                                         | phase + recommended actions
contracts                | hlv://contracts/{id} for each task.contracts[]          | full contract content
constraints              | hlv://constraints                                      | project-level constraints
dependencies             | hlv://stage/{n} → resolve each task.depends_on[]       | upstream task summaries + HANDOFF run results
handoff                  | HANDOFF runtime store                                  | upstream changed_files, notes, warnings
write_scopes             | HANDOFF scope policy (see section 4.3)                 | not from HLV directly
base_revision            | git rev-parse HEAD (at workspace creation time)        | from workspace manager
workspace                | HANDOFF runtime store                                  | workspace_id, path, kind
policy                   | HANDOFF config                                         | approval, timeout, sandbox mode
```

Maximum ambiguity rule:

- The implementation MUST document whether `contracts`, `constraints`, and handoff artifacts are embedded inline in `context_bundle` or referenced by URI/path plus digest.
- If references are used instead of embedded content, the bundle MUST include enough integrity metadata to make the read set deterministic. Minimum: canonical path/URI plus content digest or source revision.

### 11.4 Approval Policy

Approval/sandbox policy is implementation-defined, but:

- the policy MUST be explicit and documented;
- runs MUST not hang forever on approval;
- unsupported tool calls MUST fail predictably;
- the human input policy MUST be defined in advance.

---

## 12. Optional HANDOFF API Surface

The external `HANDOFF` service MAY expose its own MCP/HTTP/API surface.

Recommended operations:

- `handoff_register`
- `handoff_claim`
- `handoff_heartbeat`
- `handoff_check`
- `handoff_done`
- `handoff_fail`
- `handoff_resolve_intent`
- `handoff_status`
- `handoff_events`

Important:

- These operations are not part of the current HLV MCP.
- They belong to the external HANDOFF service.
- Internally, they may call `hlv mcp`, but they do not require changes to the HLV core.

---

## 13. Observability

### 13.1 Required Signals

- active agents
- active claims
- queued retries
- stalled runs
- conflict counts
- task throughput
- handoff events
- projection drift warnings

### 13.2 Logs

Logs SHOULD be structured and include:

- `project_id`
- `task_id`
- `stage_id`
- `milestone_id`
- `run_id`
- `agent_id`
- `session_id`
- `event_type`
- `attempt`

### 13.3 Status Surface

Recommended views:

- current ready/running/blocked tasks;
- agent occupancy;
- claims by stage;
- recent handoff/conflict events;
- drift between HLV state and runtime-store.

---

## 14. Failure Model and Recovery Strategy

### 14.1 Failure Classes

1. `HLV MCP Failures`
   - server unavailable;
   - tool call failed;
   - malformed resource payload;
   - stale projection write.

2. `Runtime Store Failures`
   - DB unavailable;
   - journal corruption;
   - lease persistence failure.

3. `Workspace Failures`
   - create/reuse failure;
   - invalid path;
   - cleanup failure.

4. `Agent Session Failures`
   - launch failed;
   - timeout;
   - stall;
   - approval deadlock;
   - unexpected exit.

5. `Conflict Engine Failures`
   - file ownership unresolved;
   - inconsistent claims;
   - propagation analysis failed.

### 14.2 Recovery Behavior

- HLV read failure:
  - skip dispatch;
  - keep existing runs when safe;
  - retry on next tick.

- HLV write failure:
  - HLV remains authoritative for project/stage/task lifecycle;
  - the runtime store MAY keep intent and recovery metadata, but does not commit lifecycle transitions on its own;
  - the affected claim/run MUST be marked as needing replay/reconciliation;
  - the orchestrator MUST retry the HLV write before treating the transition as final for scheduling decisions;
  - the orchestrator MUST replay a pending HLV lifecycle intent without relaunching the already finished worker attempt;
  - projection/lifecycle drift MUST be explicitly marked and visible to the operator.

- runtime-store failure:
  - stop new dispatches;
  - keep the process alive only if safety guarantees remain intact;
  - otherwise fail fast.

- worker failure:
  - convert to retry or terminal failure according to policy.

### 14.3 Partial State Recovery

Conformance requirement:

- after restart, the system MUST recover a consistent state without manual editing of HLV files;
- if the runtime store is lost, the orchestrator MAY use HLV state, workspace scan, and projection meta only for inventory, drift diagnostics, and preparing operator reconciliation;
- the orchestrator MUST NOT reconstruct active claims, leases, retry timers, pending lifecycle intents, or completed-but-not-promoted runs solely from HLV + workspace scan + projection meta;
- if the runtime store is lost, the system MUST enter safe degraded mode: forbid new dispatches, record ambiguity around orphaned workspaces/runs, and wait for explicit operator resolution or restoration of an intact runtime/event store;
- durable runtime-store remains mandatory for automatic recovery semantics; projection/meta bootstrap alone is insufficient by design.

Workspace-mode recovery note:

- In HLV MCP workspace mode, degraded recovery MUST be evaluated per `project_id`.
- One project entering degraded mode due to drift or runtime-store ambiguity SHOULD NOT force unrelated projects into degraded mode unless they share the same external promotion/conflict domain and the implementation explicitly documents that coupling.

---

## 15. Security and Operational Safety

### 15.1 Trust Boundary

`HANDOFF` launches agents that read code, write code, and may execute commands.

Therefore:

- the trust boundary MUST be explicitly described;
- approval/sandbox posture MUST be documented;
- workspace isolation MUST be treated as a baseline requirement.

### 15.2 Filesystem Safety

- the worker MUST write only inside its workspace;
- the orchestrator MUST validate path traversal;
- cleanup MUST use safe path checks;
- file claim rules SHOULD constrain shared writable zones.

### 15.3 Secret Handling

- secrets must not be written into `meta`;
- secrets must not be logged;
- agent context SHOULD receive capability/token access through runtime policy, not through raw file dumps.

---

## 16. Reference Algorithms

### 16.1 Service Startup

```text
function start_handoff():
  if not connect_hlv_mcp():
    fatal("Cannot connect to hlv mcp — check config and hlv installation")

  if not load_runtime_store():
    if runtime_store_path_exists():
      fatal("Runtime store corrupted — operator must restore backup or delete and restart clean")
    else:
      create_runtime_store()

  sync_result = run_hlv_task_sync()
  if sync_result.has_conflicts:
    log_warning("task_sync conflicts detected at startup")
    if not config.allow_force_sync_on_startup:
      enter_degraded_mode("task_sync conflict — operator must resolve")
      return

  check_task_id_uniqueness()
  replay_pending_intents()
  reconcile_hlv_with_runtime()
  restore_projection_state()
  schedule_tick(0)
```

Startup failure rules:

- If `hlv mcp` is unreachable, `HANDOFF` MUST fail fast. There is no useful degraded mode without the HLV backend.
- If the runtime store cannot be opened but exists on disk, `HANDOFF` MUST NOT silently recreate it — this risks orphaned claims and double-dispatches. Operator action is required.
- If the runtime store does not exist, `HANDOFF` SHOULD create it and start clean.
- If `hlv_task_sync` detects conflicts at startup, `HANDOFF` MUST either resolve them via `force` (if policy allows) or enter degraded mode.
- `replay_pending_intents()` MUST run before `reconcile_hlv_with_runtime()` to avoid treating unresolved intents as drift.

### 16.2 Poll-and-Dispatch Tick

```text
function on_tick():
  reconcile_running_runs()
  refresh_hlv_state()
  ensure_hlv_task_sync_for_current_epoch()
  ready_tasks = compute_ready_tasks()

  for task in ready_tasks:
    if no_slots():
      break
    if claim_task(task):
      project_claim_to_hlv_meta(task)
      start_worker(task)
```

### 16.3 Worker Completion

```text
function on_worker_success(run):
  if promote_workspace_changes(run) fails:
    persist_integration_intent(run)
    mark_run_status(run, "awaiting_lifecycle_commit")
    mark_projection_drift(run)
    keep_claim_recoverable(run)
    return
  collect_changed_files(run)
  propagate_impacts(run)
  if mark_hlv_task_done(run.task_ref.task_id) succeeds:
    update_projection(run, "completed")
    release_claims(run)
  else:
    persist_hlv_write_intent(run, "task_done")
    mark_run_status(run, "awaiting_lifecycle_commit")
    mark_projection_drift(run)
    keep_claim_recoverable(run)
```

### 16.4 Reconciliation

```text
function reconcile_running_runs():
  for run in active_runs:
    if lease_expired(run) or stalled(run):
      stop_run(run)
      queue_retry_or_block(run)

  drift = compare_runtime_to_hlv()
  if drift:
    repair_projection(drift)
```

---

## 17. MVP Implementation Profile

### 17.1 Purpose of MVP

A full HANDOFF system can become large. To let a developer assemble the first working version quickly through AI, the spec needs an explicit MVP profile.

MVP goal:

- safely assign ready tasks to multiple agents;
- prevent double-claiming of a task;
- provide observable runtime state;
- survive orchestrator restarts;
- project status back into HLV through `meta`;
- avoid trying to solve every semantic handoff case immediately.

### 17.2 Required MVP Capabilities

MVP MUST include:

- single orchestrator process;
- one durable runtime store;
- ready task discovery via `hlv_task_list` plus the stage/task dependency model;
- task claims with lease and retry;
- calls to `hlv_task_start`, `hlv_task_done`, and `hlv_task_block`;
- task/stage/milestone `meta` projection;
- a separate workspace per claim;
- a basic agent runner;
- structured event log;
- durable replay of ambiguous HLV lifecycle writes and promotion/integration failures;
- a reconciliation loop after restart.

MVP MAY simplify:

- file conflict detection down to path-prefix rules;
- change propagation down to coarse-grained impacted-task warnings;
- observability down to a CLI/TUI or JSON status endpoint;
- agent routing down to round-robin or a fixed agent pool.

### 17.3 Explicitly Deferred from MVP

These items can be deferred to `v2+`:

- semantic code impact analysis;
- automatic re-planning of the stage graph;
- multi-orchestrator high availability;
- complex capability marketplaces between agents;
- full event streaming UI;
- automatic patch exchange between agents;
- rich human approval workflows;
- generalized distributed lock service.

### 17.4 Recommended MVP Storage Model

Recommended pragmatic default:

- `SQLite` as the runtime store;
- tables:
  - `agents`
  - `task_claims`
  - `run_attempts`
  - `workspaces`
  - `live_sessions`
  - `handoff_events`
  - `pending_intents`
  - `retries`
  - `file_claims` (optional in MVP, but recommended if conflict handling is not purely coarse-grained and precomputed)

Minimum durable records:

- canonical task identity: `project_id`, `milestone_id`, `stage_id`, `task_id`;
- which `claim_id` currently owns that task;
- by which `run_id` and which `agent_id`;
- which workspace is assigned;
- the workspace or promotion `base_revision`;
- the status of the latest attempt;
- when the latest lease/heartbeat occurred;
- whether a retry is pending;
- any unresolved replayable `pending_intents` for `task_start`, `task_done`, `task_block`, or `promotion`;
- enough promotion metadata to retry or audit integration (`base_revision`, promotion artifact reference or digest, resulting canonical revision when available).

MVP persistence conformance note:

- An implementation that omits durable `pending_intents` is NOT restart-safe and therefore is not conformant even if everything else is implemented.
- An implementation that stores only plain `task_id` without `project_id`, `milestone_id`, and `stage_id` is NOT conformant, even in MVP.

### 17.5 Recommended MVP Meta Projection

For `task.meta`, MVP only needs:

- `handoff.state`
- `handoff.projection_version`

For `stage.meta`, MVP only needs:

- `handoff.state`
- `handoff.projection_version`

For `milestone.meta`, MVP only needs:

- `handoff.state`
- `handoff.projection_version`

Recommended `handoff.state` payloads:

- task:
  - `assignee`
  - `run_id`
  - `status`
  - `workspace`
  - `attempt`
  - `last_result`
- stage:
  - `running_tasks`
  - `blocked_tasks`
  - `last_reconcile_at`
- milestone:
  - `orchestrator_id`
  - `active_agents`
  - `active_runs`

### 17.6 Recommended MVP Scheduling Rules

MVP scheduler SHOULD:

- take new tasks only from `pending`;
- consider only already-satisfied `depends_on`;
- not start a second task if it already has an active claim;
- enforce global concurrency limits;
- when a worker errors, move the task into the retry queue instead of immediately to `blocked`.

Retry nuance for MVP:

- the first attempt of a task starts only from `pending`;
- a repeated attempt of the same task is allowed from `in_progress` if it is a retry of the same logical execution chain;
- the MVP orchestrator must distinguish `new dispatch` from `retry dispatch`, rather than treating them as the same thing.

The MVP scheduler MAY omit:

- per-state concurrency;
- advanced priority weighting;
- fairness between milestones;
- predictive scheduling.

### 17.7 Recommended MVP Conflict Model

The minimally sufficient conflict model:

1. Each task receives declared write scopes:
   - path prefixes;
   - explicit files;
   - optional shared-read areas.
2. Before a run, the orchestrator reserves the scope.
3. Exclusive scope overlap = `deny`.
4. Overlap with shared-known files = `allow_with_warning`.
5. After the run completes, changed files are stored in the event log and summary meta.

Sources of declared scopes for the MVP:

- `stage.output` as the base structural hint, if it is precise enough for the project;
- task labels;
- naming conventions;
- external config file HANDOFF;
- manual rules by stage/task ID.

Rule:

- if `stage.output` is not precise enough, HANDOFF MUST rely on external policy/config and MUST NOT declare generated write scopes as authoritative based only on the stage plan.
- the MVP implementation MUST still serialize scopes into the normalized `write_scope` structure defined in section 10.3, even if the actual policy is limited to path-prefix rules.
- MVP MAY rely entirely on pre-reserved scopes without runtime `handoff_check` if the agent runner cannot intercept individual file writes (e.g. Claude Code in `--dangerously-skip-permissions` mode). In that case, scope violations are detected post-hoc during promotion and treated as integration conflicts.

### 17.8 Recommended MVP Configuration Format

The exact config format is implementation-defined. Reference example:

```yaml
# handoff.yaml
hlv_mcp:
  transport: stdio             # stdio | sse
  command: hlv
  args: [mcp]
  cwd: /path/to/project
  # workspace mode:
  # args: [mcp, --workspace, /path/to/workspace.yaml]

runtime_store:
  path: .handoff/runtime.db    # SQLite path, relative to HANDOFF working dir

workspace:
  root: .handoff/workspaces    # base directory for worktrees/sandboxes
  kind: git_worktree           # git_worktree | directory_copy
  cleanup_on_release: true

scheduling:
  tick_interval_ms: 5000
  max_global_concurrency: 4
  max_per_stage_concurrency: 2
  task_sync_before_each_tick: true  # recommended for MVP

retry:
  max_attempts: 3
  initial_delay_ms: 10000
  max_delay_ms: 300000
  backoff_multiplier: 2.0

lease:
  ttl_seconds: 300
  heartbeat_interval_seconds: 30
  stale_threshold_seconds: 120

write_policy:
  default_mode: exclusive
  shared_paths: []             # glob patterns for shared-write zones
  # Example: ["src/shared/**", "proto/**"]

agents:
  - id: agent-1
    kind: claude-code           # implementation-defined agent kind
    workspace_root: .handoff/workspaces/agent-1
    capabilities: [rust, typescript]

projection:
  enabled: true
  on_transitions_only: true    # project to meta only on state transitions, not heartbeats
```

### 17.9 Recommended MVP Build Order

1. `HLV MCP adapter`
   - reading tasks/milestones/workflow;
   - calling `hlv_task_*` and `*_meta`.

2. `Runtime store`
   - claims, attempts, retries, agent heartbeats.

3. `Orchestrator loop`
   - poll, claim, run, reconcile, retry.

4. `Workspace manager`
   - create, reuse, cleanup, path safety.

5. `Agent runner`
   - launch, collect events, stop on timeout.

6. `Meta projection`
   - writing summary state back into HLV.

7. `Conflict MVP`
   - path-prefix scope checks.

8. `Status surface`
   - logs + simple status command/API.

### 17.10 MVP Definition of Done

The MVP is considered done if the system can:

- restart without losing active claims;
- run at least 2 agents in parallel on independent tasks;
- prevent double-claiming of one HLV task;
- complete a task via `hlv_task_done` after a successful run;
- block or requeue a run on scope conflict;
- show the operator who is currently executing which task;
- repair drift between the runtime store and `meta`.

---

## 18. Reference Persistence Schema

### 18.1 Storage Choice

Recommended default:

- `SQLite` in WAL mode;
- single writer orchestrator process;
- read access for status/API processes;
- periodic backup or snapshot export optional.

`SQLite` fits `HANDOFF` because:

- it requires no separate infrastructure;
- it is sufficiently reliable for a single-host orchestrator;
- it is convenient for restart recovery;
- it fits the event log + lease model well.

### 18.2 Tables

#### `agents`

Purpose:

- registry of known agents and their heartbeat state.

Columns:

- `agent_id TEXT PRIMARY KEY`
- `agent_kind TEXT NOT NULL`
- `status TEXT NOT NULL`
- `capabilities_json TEXT NOT NULL DEFAULT '{}'`
- `workspace_root TEXT`
- `current_run_id TEXT`
- `last_heartbeat_at TEXT`
- `registered_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

Indexes:

- `idx_agents_status(status)`
- `idx_agents_last_heartbeat_at(last_heartbeat_at)`

#### `task_claims`

Purpose:

- current and historical claims for HLV tasks.

Columns:

- `claim_id TEXT PRIMARY KEY`
- `project_id TEXT NOT NULL`
- `milestone_id TEXT NOT NULL`
- `stage_id INTEGER NOT NULL`
- `task_id TEXT NOT NULL`
- `agent_id TEXT NOT NULL`
- `active_run_id TEXT`
- `status TEXT NOT NULL`
- `claim_version INTEGER NOT NULL DEFAULT 1`
- `lease_expires_at TEXT`
- `claimed_at TEXT NOT NULL`
- `released_at TEXT`
- `release_reason TEXT`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

Indexes:

- `idx_task_claims_task(project_id, milestone_id, stage_id, task_id, status)`
- `idx_task_claims_active_run(active_run_id)`
- `idx_task_claims_agent(agent_id, status)`
- `idx_task_claims_lease(lease_expires_at, status)`

Constraint:

- there must be a partial uniqueness invariant: no more than one active claim on `(project_id, milestone_id, stage_id, task_id)`.

#### `run_attempts`

Purpose:

- one row per run attempt.

Columns:

- `claim_id TEXT NOT NULL`
- `run_id TEXT PRIMARY KEY`
- `agent_id TEXT NOT NULL`
- `project_id TEXT NOT NULL`
- `milestone_id TEXT NOT NULL`
- `stage_id INTEGER NOT NULL`
- `task_id TEXT NOT NULL`
- `attempt INTEGER NOT NULL`
- `workspace_id TEXT`
- `status TEXT NOT NULL`
- `error_code TEXT`
- `error_message TEXT`
- `retry_count INTEGER NOT NULL DEFAULT 0`
- `started_at TEXT NOT NULL`
- `finished_at TEXT`
- `last_event_at TEXT`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

Indexes:

- `idx_run_attempts_task(project_id, milestone_id, stage_id, task_id, status)`
- `idx_run_attempts_agent(agent_id, status)`
- `idx_run_attempts_last_event(last_event_at)`

#### `live_sessions`

Purpose:

- volatile-but-durable metadata for live agent sessions.

Columns:

- `session_id TEXT PRIMARY KEY`
- `run_id TEXT NOT NULL`
- `thread_id TEXT`
- `turn_id TEXT`
- `pid INTEGER`
- `status TEXT NOT NULL`
- `input_tokens INTEGER NOT NULL DEFAULT 0`
- `output_tokens INTEGER NOT NULL DEFAULT 0`
- `total_tokens INTEGER NOT NULL DEFAULT 0`
- `last_event_at TEXT`
- `started_at TEXT NOT NULL`
- `ended_at TEXT`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

Indexes:

- `idx_live_sessions_run(run_id)`
- `idx_live_sessions_status(status)`

#### `workspaces`

Purpose:

- tracks workspace lifecycle (creation, reuse, cleanup).

Columns:

- `workspace_id TEXT PRIMARY KEY`
- `workspace_path TEXT NOT NULL`
- `workspace_kind TEXT NOT NULL` (`git_worktree` | `directory_copy`)
- `claim_id TEXT NOT NULL`
- `project_id TEXT NOT NULL`
- `milestone_id TEXT NOT NULL`
- `stage_id INTEGER NOT NULL`
- `task_id TEXT NOT NULL`
- `agent_id TEXT NOT NULL`
- `base_revision TEXT`
- `reused INTEGER NOT NULL DEFAULT 0`
- `status TEXT NOT NULL`
- `created_at TEXT NOT NULL`
- `cleaned_at TEXT`
- `updated_at TEXT NOT NULL`

Indexes:

- `idx_workspaces_claim(claim_id)`
- `idx_workspaces_status(status)`

#### `file_claims`

Purpose:

- runtime rights for write scopes.

Columns:

- `file_claim_id TEXT PRIMARY KEY`
- `run_id TEXT NOT NULL`
- `scope_kind TEXT NOT NULL`
- `scope_value TEXT NOT NULL`
- `mode TEXT NOT NULL`
- `status TEXT NOT NULL`
- `declared_at TEXT NOT NULL`
- `released_at TEXT`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

Indexes:

- `idx_file_claims_run(run_id, status)`
- `idx_file_claims_scope(scope_kind, scope_value, status)`

#### `handoff_events`

Purpose:

- append-only event log for observability and recovery.

Columns:

- `event_id TEXT PRIMARY KEY`
- `event_type TEXT NOT NULL`
- `claim_id TEXT`
- `run_id TEXT`
- `agent_id TEXT`
- `project_id TEXT`
- `milestone_id TEXT`
- `stage_id INTEGER`
- `task_id TEXT`
- `session_id TEXT`
- `payload_json TEXT NOT NULL DEFAULT '{}'`
- `created_at TEXT NOT NULL`

Indexes:

- `idx_handoff_events_task(project_id, milestone_id, stage_id, task_id, created_at)`
- `idx_handoff_events_claim(claim_id, created_at)`
- `idx_handoff_events_run(run_id, created_at)`
- `idx_handoff_events_type(event_type, created_at)`

#### `pending_intents`

Purpose:

- durable replay and operator-resolution queue for lifecycle and promotion actions whose outcome is not yet final.

Columns:

- `intent_id TEXT PRIMARY KEY`
- `intent_kind TEXT NOT NULL` (`task_start` | `task_done` | `task_block` | `promotion`)
- `claim_id TEXT`
- `run_id TEXT`
- `project_id TEXT NOT NULL`
- `milestone_id TEXT NOT NULL`
- `stage_id INTEGER NOT NULL`
- `task_id TEXT NOT NULL`
- `idempotency_key TEXT NOT NULL`
- `status TEXT NOT NULL`
- `payload_json TEXT NOT NULL DEFAULT '{}'`
- `not_before TEXT`
- `last_error TEXT`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`
- `resolved_at TEXT`

Indexes:

- `idx_pending_intents_status(status, not_before)`
- `idx_pending_intents_task(project_id, milestone_id, stage_id, task_id, status)`
- `idx_pending_intents_claim(claim_id, status)`

#### `retries`

Purpose:

- delayed retry timers and continuation retries.

Columns:

- `retry_id TEXT PRIMARY KEY`
- `claim_id TEXT NOT NULL`
- `run_id TEXT`
- `project_id TEXT NOT NULL`
- `milestone_id TEXT NOT NULL`
- `stage_id INTEGER NOT NULL`
- `task_id TEXT NOT NULL`
- `attempt INTEGER NOT NULL`
- `reason TEXT`
- `due_at TEXT NOT NULL`
- `status TEXT NOT NULL`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

Indexes:

- `idx_retries_due(due_at, status)`
- `idx_retries_task(project_id, milestone_id, stage_id, task_id, status)`

### 18.3 Status Enums and Event Types

#### Canonical Event Types

Orchestrator events (`handoff_events.event_type`):

- `agent_registered` — agent registered or refreshed
- `agent_heartbeat` — agent heartbeat received (optional, may be high-volume)
- `agent_stale` — agent marked stale due to missed heartbeats
- `claim_created` — new task claim established
- `claim_released` — claim released after completion or terminal failure
- `claim_expired` — claim expired due to lease timeout
- `claim_superseded` — claim replaced by operator recovery
- `run_started` — run attempt launched
- `run_succeeded` — run attempt completed successfully
- `run_failed` — run attempt failed
- `run_timed_out` — run attempt exceeded timeout
- `run_stalled` — run attempt detected as stalled
- `run_cancelled` — run attempt cancelled by reconciliation
- `retry_queued` — retry scheduled for a failed run
- `retry_fired` — retry timer expired, new attempt starting
- `hlv_task_started` — `hlv_task_start` successfully called
- `hlv_task_completed` — `hlv_task_done` successfully called
- `hlv_task_blocked` — `hlv_task_block` successfully called
- `hlv_lifecycle_failed` — HLV lifecycle write failed, intent persisted
- `hlv_lifecycle_replayed` — pending HLV lifecycle intent successfully replayed
- `promotion_succeeded` — workspace changes promoted to canonical tree
- `promotion_failed` — promotion/integration failed, intent persisted
- `conflict_detected` — file scope conflict detected
- `file_claim_granted` — file scope reserved for a run
- `file_claim_denied` — file scope request denied
- `file_claim_released` — file scope released
- `projection_updated` — meta projection written to HLV
- `projection_drift` — drift detected between runtime store and HLV meta
- `task_sync_completed` — `hlv_task_sync` completed
- `task_sync_conflict` — `hlv_task_sync` detected conflicts
- `task_id_ambiguity` — duplicate `task_id` detected in active milestone
- `degraded_mode_entered` — orchestrator entered degraded mode
- `degraded_mode_exited` — orchestrator exited degraded mode
- `intent_resolved` — pending intent resolved by operator

Runner events (emitted by the agent runner, see section 11.2):

- `session_started`
- `progress`
- `file_intent_declared`
- `conflict_detected`
- `handoff_requested`
- `turn_completed`
- `turn_failed`
- `run_succeeded`
- `run_failed`

Implementations MAY add custom event types with a namespaced prefix (e.g. `custom.my_event`), but MUST NOT redefine the canonical types above.

#### Status Enums

Recommended normalized statuses:

- `agents.status`: `idle`, `running`, `offline`, `stale`, `disabled`
- `task_claims.status`: `active`, `released`, `expired`, `superseded`
- `run_attempts.status`: `preparing`, `running`, `waiting_handoff`, `awaiting_lifecycle_commit`, `retry_queued`, `succeeded`, `failed`, `timed_out`, `stalled`, `cancelled`
- `live_sessions.status`: `initializing`, `streaming`, `completed`, `failed`, `cancelled`
- `workspaces.status`: `active`, `promoting`, `cleaned`, `orphaned`
- `file_claims.status`: `active`, `released`, `denied`
- `pending_intents.status`: `pending`, `replaying`, `blocked_for_operator`, `resolved`, `cancelled`
- `retries.status`: `pending`, `fired`, `cancelled`, `consumed`

### 18.4 Suggested DDL Sketch

Illustrative SQL sketch:

```sql
CREATE TABLE task_claims (
  claim_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  milestone_id TEXT NOT NULL,
  stage_id INTEGER NOT NULL,
  task_id TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  active_run_id TEXT,
  status TEXT NOT NULL,
  claim_version INTEGER NOT NULL DEFAULT 1,
  lease_expires_at TEXT,
  claimed_at TEXT NOT NULL,
  released_at TEXT,
  release_reason TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX uq_task_claims_active_task
ON task_claims(project_id, milestone_id, stage_id, task_id)
WHERE status = 'active';
```

### 18.5 Persistence Rules

- `handoff_events` SHOULD be append-only.
- Lease renewal MUST update `task_claims.updated_at` and `lease_expires_at`.
- Retry dispatch MUST atomically create a new `run_attempts` row, update `task_claims.active_run_id`, and record the new attempt counter.
- All replay, retry, and uniqueness checks MUST be scoped by `project_id` as well as by milestone/stage/task identity.
- Worker terminal transitions MUST atomically:
  - close `run_attempts`;
  - release active `file_claims`;
  - emit `handoff_events`;
  - schedule or cancel retry.
- Failed or outcome-ambiguous HLV lifecycle writes (`task_start`, `task_done`, `task_block`) MUST be persisted in `pending_intents` as recoverable intents until replay succeeds or the operator explicitly resolves them.
- Promotion/integration failures that occur after worker completion MUST also be persisted in `pending_intents` with `intent_kind = promotion`.
- Every replayable intent MUST carry a stable `idempotency_key` so reconciliation can retry the same logical effect without forking a new execution chain.
- Reconciliation MUST consume `pending_intents` before creating new claims for the same task.
- Projection drift MUST be logged as an event, not merely as a transient warning.
- Projection writes SHOULD preserve serialized `handoff.state` and `handoff.projection_version` as one logical projection unit.

---

## 19. External HANDOFF API Contract

### 19.1 Scope

This API is not part of `hlv mcp`. It is the external contract of the `HANDOFF` service, which:

- coordinates agents;
- uses `hlv mcp` internally;
- provides its own orchestration surface to agents and operators.

The API may be implemented as:

- HTTP JSON API;
- its own MCP server;
- both at the same time.

The logical contract is described below. The transport may differ as long as the semantics are preserved.

Control-mode note:

- Primary architecture for `HANDOFF v1` = orchestrator-driven dispatch: the orchestrator itself chooses the eligible task, creates the claim/run, and launches the worker.
- The API described below is also compatible with pull-mode agent pools, where an agent requests work through `handoff_claim`.
- In pull mode, `handoff_claim` is only a transport adapter to the same orchestrator-owned scheduler and MUST use the same eligibility, lease, idempotency, and single-writer rules.
- The implementation MUST choose one primary deployment mode and document it; if both modes are supported, orchestrator-driven semantics remain authoritative.
- If `HANDOFF` is backed by HLV MCP workspace mode, the API/runtime contract MUST preserve `project_id` through scheduling, persistence, and reconciliation.

### 19.2 Core Operations

#### `handoff_register`

Purpose:

- register an agent or refresh its presence.

Input:

- `agent_id`
- `agent_kind`
- `capabilities`
- `workspace_root`
- `supports_file_claims`
- `supports_handoff_resume`

Output:

- `registered: true`
- `agent_status`
- `lease_ttl_seconds`
- `server_time`

Effects:

- upsert into `agents`;
- event `agent_registered`.

#### `handoff_claim`

Purpose:

- atomically assign the next eligible task to an agent, or a specific task by ID.

Input:

- `project_id?`
- `agent_id`
- `task_id?`
- `stage_id?`
- `capability_filter?`
- `dry_run?`

Task targeting rule:

- in single-project HLV MCP mode, `project_id` is ignored;
- in workspace HLV MCP mode, the implementation MUST either:
  - require explicit `project_id`; or
  - document one deterministic cross-project scheduling policy for omitted `project_id`;
- even when a cross-project scheduler exists, the returned `task_ref` MUST always include the resolved `project_id`;
- if an explicit `task_id` is provided, the implementation SHOULD treat it as a request hint rather than bypassing scheduler invariants;
- if explicit targeting points to a non-unique or lifecycle-ambiguous task inside the active milestone, the operation MUST fail with a deterministic error rather than selecting an arbitrary stage instance;
- `handoff_claim` MUST NOT mask ambiguity of plain `task_id`, because downstream HLV lifecycle writes are still addressed by `task_id`.

`dry_run` semantics:

- if `dry_run = true`, the operation MUST be a read-only preview;
- it MUST NOT create `task_claims`, `run_attempts`, `file_claims`, or projection writes;
- it MUST NOT call `hlv_task_start`;
- it MAY return the candidate task/context bundle and the `dispatch_kind` that would be chosen for a real claim;
- if there is no eligible task, the response remains `no_eligible_task`.

Output on success:

- `claim_id`
- `run_id`
- `dispatch_kind`: `new` | `retry`
- `task_ref`
- `attempt`
- `workspace`
- `context_bundle`
- `lease_expires_at`
- `write_scopes`

`write_scopes` contract:

- each entry MUST use the normalized structure from section 10.3;
- the response SHOULD include only the scopes currently granted/reserved for the run, not speculative future scopes;
- if a scope is advisory rather than authoritative, `authoritative` MUST be `false`.

Reference response example:

```json
{
  "claim_id": "claim-20260309-xyz",
  "run_id": "run-20260309-001",
  "dispatch_kind": "new",
  "task_ref": {
    "project_id": "my-project",
    "milestone_id": "001-order-create",
    "stage_id": 1,
    "task_id": "TASK-002"
  },
  "attempt": 1,
  "workspace": {
    "workspace_id": "ws-abc123",
    "workspace_path": "/workspaces/claim-20260309-xyz/",
    "workspace_kind": "git_worktree"
  },
  "context_bundle": { "...": "see section 11.2" },
  "lease_expires_at": "2026-03-09T12:05:00Z",
  "write_scopes": [
    {
      "scope_kind": "path_prefix",
      "scope_value": "src/features/order_create/",
      "mode": "exclusive",
      "source": "stage_output",
      "authoritative": true
    }
  ]
}
```

Output when no task available:

- `claim_id: null`
- `reason: "no_eligible_task"`

Effects:

- create active claim for `dispatch_kind = new`, or reuse existing active `claim_id` for `dispatch_kind = retry`;
- create run attempt;
- reserve declared `write_scopes` before worker launch when scope policy can determine them upfront; if scopes are partially dynamic, create the initial reservation set and require later `handoff_check` for expansion;
- call `hlv_task_start` only for `dispatch_kind = new`;
- if `hlv_task_start` returns an outcome-ambiguous error, persist a `pending_intents` record for `task_start`, keep the claim recoverable, and do not launch the worker until reconciliation resolves the start outcome;
- write task meta projection.

Effects when `dry_run = true`:

- no durable mutations;
- no HLV lifecycle calls;
- no lease acquisition;
- no concurrency slot consumption beyond the request lifetime.

#### `handoff_heartbeat`

Purpose:

- renew the lease and update live runtime state.

Input:

- `agent_id`
- `run_id`
- `session_id?`
- `status`
- `last_event_at`
- `input_tokens?`
- `output_tokens?`
- `total_tokens?`

Output:

- `ok`
- `lease_expires_at`
- `server_actions`

Possible `server_actions`:

- `continue`
- `refresh_context`
- `pause_for_handoff`
- `stop`

Effects:

- update `agents`, `run_attempts`, `live_sessions`;
- optionally emit heartbeat event;
- may trigger stale/run policy.

#### `handoff_check`

Purpose:

- validate write intent before changing a file or scope.

Input:

- `agent_id`
- `run_id`
- `paths`
- `mode`

Reference request example:

```json
{
  "agent_id": "agent-1",
  "run_id": "run-20260309-001",
  "paths": ["src/shared/types.rs", "src/shared/mod.rs"],
  "mode": "exclusive"
}
```

Output:

- `decision`: `allow` | `allow_with_warning` | `deny`
- `conflicts`
- `required_handoff`

Reference response example (`deny`):

```json
{
  "decision": "deny",
  "conflicts": [
    {
      "claim_id": "claim-20260309-abc",
      "run_id": "run-20260309-002",
      "scope_kind": "path_prefix",
      "scope_value": "src/shared/",
      "mode": "exclusive",
      "reason": "Active exclusive claim by agent-2 on TASK-003"
    }
  ],
  "required_handoff": [
    {
      "project_id": "my-project",
      "milestone_id": "001-order-create",
      "stage_id": 1,
      "task_id": "TASK-003",
      "run_id": "run-20260309-002"
    }
  ]
}
```

Reference response example (`allow`):

```json
{
  "decision": "allow",
  "conflicts": [],
  "required_handoff": []
}
```

Effects:

- may create or deny `file_claims`;
- emit `conflict_detected` when needed.

Contract note:

- `conflicts` and `required_handoff` MUST follow the structured contract defined in section 10.3 and MUST NOT be plain diagnostic strings only.

#### `handoff_done`

Purpose:

- complete the run as successful and return the result to the orchestrator.

Input:

- `agent_id`
- `run_id`
- `session_id?`
- `changed_files`
- `summary`
- `handoff_notes?`
- `artifacts?`

`changed_files` semantics:

- the worker-provided list MAY be incomplete or advisory;
- the orchestrator MUST validate or recompute the authoritative changed file set server-side before impact analysis and lifecycle commit.

Output:

- `ok`
- `task_status`
- `propagation_actions`

Effects:

- promote/integrate authoritative workspace diff into canonical project state;
- recompute or validate authoritative changed file set from the promoted state or the promotion artifact used to update canonical state;
- impact analysis;
- emit completion events;
- call `hlv_task_done` only after successful promotion/integration;
- only after successful `hlv_task_done` mark the claim as fully completed in HLV-facing projection;
- if `hlv_task_done` fails, persist replay intent in `pending_intents`, surface lifecycle drift and keep the claim recoverable until reconciliation finishes;
- if promotion/integration fails, persist a `promotion` pending intent or operator-required recovery marker and keep the claim recoverable without marking the task done in HLV;
- update projection;
- release claims only after successful HLV lifecycle commit or explicit operator resolution;
- create downstream handoff actions if needed.

#### `handoff_fail`

Purpose:

- complete the run as a failure, timeout, or blocked result.

Input:

- `agent_id`
- `run_id`
- `failure_kind`
- `error_code`
- `error_message`
- `retryable`
- `changed_files?`

`changed_files` semantics:

- if the worker reports `changed_files` even on failure, that data remains advisory until server-side diff/reconciliation;
- the orchestrator SHOULD store both the reported and authoritative file sets in the event log if they differ.

Output:

- `ok`
- `next_state`: `retry_queued` | `blocked` | `failed_terminal`
- `retry_due_at?`

Effects:

- close run attempt;
- if `retryable = true`, requeue the same `claim_id` with a new future attempt;
- if `retryable = false`, map terminal failure to `hlv_task_block`;
- if terminal `hlv_task_block` succeeds, release claim and finalize HLV-facing projection;
- if terminal `hlv_task_block` fails, persist replay intent in `pending_intents`, surface lifecycle drift and keep the claim recoverable until reconciliation or operator resolution;
- update projection;
- release file scopes and sessions as needed.

#### `handoff_resolve_intent`

Purpose:

- allow an operator or admin controller to resolve a blocked replay/promotion intent without editing HLV files by hand.

Input:

- `intent_id`
- `action`: `retry_now` | `cancel_claim` | `mark_resolved`
- `note?`

Output:

- `ok`
- `intent_status`
- `claim_status?`

Effects:

- update the targeted `pending_intents` row;
- optionally trigger immediate replay/reconciliation for `retry_now`;
- if `cancel_claim` is chosen, finalize the runtime claim in a documented operator-resolved state without pretending that a missing HLV lifecycle write succeeded.

#### `handoff_status`

Purpose:

- retrieve an operator snapshot.

Input:

- `project_id?`
- `agent_id?`
- `task_id?`
- `stage_id?`
- `include_events?`

Output:

- `agents`
- `active_claims`
- `active_runs`
- `pending_intents`
- `queued_retries`
- `blocked_tasks`
- `recent_events`
- `projection_drift`

#### `handoff_events`

Purpose:

- read the event log paginated or by cursor.

Input:

- `after_event_id?`
- `limit?`
- `project_id?`
- `task_id?`
- `run_id?`
- `event_type?`

Output:

- `events`
- `next_cursor`

### 19.3 Error Model

Recommended error codes:

- `project_id_required`
- `project_not_found`
- `agent_not_registered`
- `task_not_found`
- `task_not_eligible`
- `task_id_ambiguous`
- `task_already_claimed`
- `hlv_task_start_failed`
- `hlv_task_start_unknown`
- `lease_expired`
- `invalid_run_id`
- `conflict_denied`
- `promotion_conflict`
- `intent_not_found`
- `operator_action_required`
- `projection_write_failed`
- `runtime_store_unavailable`
- `internal_error`

### 19.4 Idempotency Rules

- `handoff_register` MUST be idempotent by `agent_id`.
- `handoff_heartbeat` MUST be idempotent for duplicate timestamps.
- `handoff_done` MUST tolerate duplicate delivery for the same `run_id`.
- `handoff_fail` MUST tolerate duplicate delivery for the same terminal `run_id`.
- `handoff_claim` MUST never create two active claims for one task.
- duplicate `handoff_claim` on the same retryable task MUST reuse the same active `claim_id` or fail deterministically; it MUST NOT fork a second execution chain.

### 19.5 HTTP Mapping

Recommended HTTP mapping:

- `POST /api/v1/register` -> `handoff_register`
- `POST /api/v1/claim` -> `handoff_claim`
- `POST /api/v1/heartbeat` -> `handoff_heartbeat`
- `POST /api/v1/check` -> `handoff_check`
- `POST /api/v1/done` -> `handoff_done`
- `POST /api/v1/fail` -> `handoff_fail`
- `POST /api/v1/intents/resolve` -> `handoff_resolve_intent`
- `GET /api/v1/status` -> `handoff_status`
- `GET /api/v1/events` -> `handoff_events`

### 19.6 MCP Mapping

If HANDOFF exposes its own MCP server, recommended tool names:

- `handoff_register`
- `handoff_claim`
- `handoff_heartbeat`
- `handoff_check`
- `handoff_done`
- `handoff_fail`
- `handoff_resolve_intent`
- `handoff_status`
- `handoff_events`

Recommended read-only resources:

- `handoff://agents`
- `handoff://claims`
- `handoff://runs`
- `handoff://intents`
- `handoff://events`
- `handoff://status`

---

## 20. Implementation Checklist

### 20.1 Required for Conformance

- The external service uses only public `hlv mcp` to access HLV.
- The HLV core is not modified.
- There is a single-writer orchestrator.
- There is a durable runtime store.
- At startup and after every resync/drift event, there is a preflight check that all dispatchable `task_id` values are unique within the active milestone of each addressed project, or dispatch is moved into an operator-required degraded state for the affected project.
- In workspace-backed deployments, all dispatch, retry, replay, and uniqueness decisions are correctly scoped by `project_id`.
- There is a task claim model.
- There is a run/session model.
- There is a reconciliation loop.
- There is a retry/backoff policy.
- There are workspace safety checks.
- There is an explicit promotion/integration step from the isolated workspace into canonical project state.
- There is a `meta` projection contract.
- There is at least minimal conflict detection.
- There are structured logs.
- There is a durable `pending_intents` mechanism for replaying ambiguous HLV lifecycle writes and failed promotions.
- There is an operator-facing intent resolution mechanism (e.g. `handoff_resolve_intent`).

### 20.2 Recommended Extensions

- dashboard/API;
- semantic impact analysis;
- capability-based agent routing;
- partial auto-handoff prompts;
- richer event timeline;
- metrics export.

### 20.3 Known Limits

- Current HLV MCP does not provide native lease/heartbeat semantics.
- Current HLV MCP does not provide file-claim primitives.
- Current HLV MCP subscriptions are useful, but do not replace a runtime event bus.
- `meta` is string-based and must not be used as the only orchestration database.

---

## 21. Summary

`HANDOFF` is implementable on top of the current `hlv mcp` without changes to the HLV core.

To do this, the following architectural split must be accepted:

- `HLV` = authoritative project/task graph;
- `HANDOFF` = authoritative agent/runtime orchestration;
- `meta` = the projection bridge between them.

This approach preserves the cleanliness of the HLV core while enabling a full orchestration system: bounded concurrency, workspace isolation, retries, reconciliation, handoff propagation, and an observable multi-agent runtime.
