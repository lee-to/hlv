# HLV Dashboard Web UI Specification

Status: Draft v2

Purpose: define a simple web dashboard on top of `hlv mcp` so a developer can build a browser UI for one HLV project or an HLV workspace with realtime updates.

Companion docs (must read before implementing):

- `docs/MCP.md` — MCP server API reference: transport details, JSON-RPC request/response examples, SSE connection code, workspace URI scheme, notification format.

---

## 1. Intent

This dashboard is a web version of `hlv dashboard`, not a new orchestration system.

It should:

- show the current state of HLV projects;
- work with one project or a workspace of many projects;
- update automatically when HLV state changes;
- use only existing HLV MCP resources/tools;
- stay simple.

It should not:

- introduce a new backend or database;
- duplicate HLV state;
- invent extra workflow states;
- require schema changes in HLV;
- try to solve handoff/orchestration problems.

---

## 2. Architecture

The dashboard talks directly to:

- `hlv mcp --transport sse`

Recommended launch:

```bash
hlv mcp --transport sse --port 3000
hlv mcp --workspace ~/.hlv/workspace.yaml --transport sse --port 3000
```

Default assumption:

- browser frontend only;
- direct connection to HLV MCP over HTTP + SSE;
- no custom API server in MVP.

If a team later wants auth/proxying, they can add a thin gateway, but that is outside this spec.

### 2.1 Transport contract

HLV MCP uses the **MCP Streamable HTTP** transport. All communication goes through a single endpoint:

- `POST /mcp` — JSON-RPC 2.0 requests; responses arrive as `text/event-stream` (SSE);
- `GET /mcp` — reconnect to an existing SSE session (requires `Mcp-Session-Id`);
- `DELETE /mcp` — close session.

CORS is fully open (all origins, methods, headers).

**Important:** every request must include the header `Accept: application/json, text/event-stream`. The server returns `406 Not Acceptable` without it.

**Important:** all responses (including POST) are `text/event-stream`. Each JSON-RPC message is delivered as an SSE `data:` line:

```
data: {"jsonrpc":"2.0","id":1,"result":{...}}

```

You cannot use `response.json()` directly — you must parse SSE frames first.

### 2.2 Connection lifecycle

Step-by-step flow for a browser client:

1. `POST /mcp` — send `initialize` request with `Accept: application/json, text/event-stream`; the server returns a **long-lived SSE stream**; read the `Mcp-Session-Id` response header; parse the first `data:` line as the initialize response; **keep this connection open** — it is also the notification channel;
2. `POST /mcp` — send `notifications/initialized` (no `id`, it is a notification; include `Mcp-Session-Id` header);
3. `POST /mcp` — send `resources/subscribe` for each URI you want to watch;
4. The SSE stream from step 1 receives `notifications/resources/updated` events as files change;
5. `DELETE /mcp` — close the session when done; alternatively, closing the SSE stream from step 1 also ends the session.

All requests after step 1 must include the `Mcp-Session-Id` header.

If the SSE stream from step 1 drops, `GET /mcp` with the same `Mcp-Session-Id` reconnects to the existing session. If the session has already expired, start over from step 1.

Alternatively, use an MCP client SDK (e.g. `@modelcontextprotocol/sdk`) which handles the lifecycle automatically. See `docs/MCP.md` for connection code examples.

### 2.3 MCP initialize handshake

The `initialize` request in step 1 above:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2025-03-26",
    "capabilities": {
      "roots": { "listChanged": false }
    },
    "clientInfo": {
      "name": "hlv-dashboard",
      "version": "0.1.0"
    }
  }
}
```

The server responds (inside the SSE stream) with its capabilities (tools, resources, subscriptions). After receiving the response, the client must send `notifications/initialized` to complete the handshake.

To subscribe to a resource for realtime updates:

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "resources/subscribe",
  "params": { "uri": "hlv://milestones" }
}
```

When the subscribed resource changes, the server pushes a `notifications/resources/updated` event over SSE with the URI. The client must then refetch the resource.

---

## 3. Source of Truth

HLV remains the only source of truth.

The dashboard must:

- read visible state from MCP resources;
- perform mutations only through MCP tools;
- treat browser state as cache only.

The dashboard must not:

- keep its own canonical project/task store;
- write hidden metadata to HLV;
- guess values that are missing from HLV.

---

## 4. Supported Modes

### 4.1 Single-project mode

Fixed resources:

| Resource | Description |
|----------|-------------|
| `hlv://project` | Project configuration and metadata |
| `hlv://milestones` | Current milestone + history |
| `hlv://workflow` | Current phase and next actions |
| `hlv://tasks` | All tasks across stages |
| `hlv://contracts` | Contract list (id + formats) |
| `hlv://gates` | Validation gates policy |
| `hlv://constraints` | Rule-based constraints |
| `hlv://plan` | Implementation plan (markdown) |
| `hlv://artifacts` | Artifact metadata index (not rendered in MVP) |
| `hlv://glossary` | Domain glossary (not rendered in MVP) |
| `hlv://map` | LLM code map (not rendered in MVP) |
| `hlv://traceability` | Requirement → contract → code links (not rendered in MVP) |

Parameterized resource templates:

| Template | Description |
|----------|-------------|
| `hlv://stage/{n}` | Stage plan by number |
| `hlv://contracts/{id}` | Single contract details |
| `hlv://tasks/{n}` | Tasks filtered by stage number |
| `hlv://artifacts/{name}` | Single global artifact |
| `hlv://artifacts/milestone/{mid}` | Milestone artifact index |
| `hlv://artifacts/milestone/{mid}/{name}` | Single milestone artifact |

### 4.2 Workspace mode

Workspace list resource:

| Resource | Description |
|----------|-------------|
| `hlv://projects` | Lightweight summary of all projects |

All single-project resources become project-scoped templates:

- `hlv://projects/{id}` — project read (canonical)
- `hlv://projects/{id}/workflow`
- `hlv://projects/{id}/milestones`
- `hlv://projects/{id}/tasks`
- `hlv://projects/{id}/stage/{n}`
- `hlv://projects/{id}/contracts`
- `hlv://projects/{id}/contracts/{cid}`
- `hlv://projects/{id}/gates`
- `hlv://projects/{id}/constraints`
- `hlv://projects/{id}/plan`
- `hlv://projects/{id}/artifacts`
- `hlv://projects/{id}/artifacts/{name}`
- `hlv://projects/{id}/artifacts/milestone/{mid}`
- `hlv://projects/{id}/artifacts/milestone/{mid}/{name}`
- `hlv://projects/{id}/glossary`
- `hlv://projects/{id}/map`
- `hlv://projects/{id}/traceability`
- `hlv://projects/{id}/tasks/{n}`

Rule:

- in workspace mode every MCP tool call must include `project_id`.

Implementation advice:

- normalize single-project mode to "workspace with one project" in the UI.

No active milestone rule:

- `hlv://workflow`, `hlv://project`, `hlv://milestones`, and `hlv://gates` are safe to read with no active milestone;
- `hlv://tasks`, `hlv://contracts`, `hlv://stage/{n}`, and task-mutating flows depend on an active milestone and may return `No active milestone`.

---

## 5. MVP Screens

The MVP should have 5 main screens.

### Navigation

| Route | Screen | Notes |
|-------|--------|-------|
| `/` | Workspace | Project list; in single-project mode redirects to `/projects/{id}` |
| `/projects/{id}` | Project Overview | |
| `/projects/{id}/plan` | Plan | Stages and tasks |
| `/projects/{id}/contracts` | Contracts / Gates | |
| `/projects/{id}/constraints` | Constraints | |

Breadcrumb: `Workspace → Project → {Screen}`.

### 5.1 Workspace

Shown in workspace mode, and optionally reused in single-project mode.

For each project show:

- project id;
- project name;
- current milestone id;
- milestone status;
- stages done / total;
- task counts: pending, in progress, done, blocked;
- workflow phase if available.

Data source rule:

- `hlv://projects` gives only the lightweight project summary: id, root, name, current milestone, milestone status, and stage progress;
- task counts and workflow phase for the workspace screen require additional per-project reads of `tasks` and `workflow`.
- if a project has no active milestone, do not treat `tasks`/`contracts` errors as fatal for the workspace page; show zero/empty task data and the workflow empty state instead.

User can:

- open a project page;
- refresh data.

### 5.2 Project Overview

Show:

- project name;
- root path if available from workspace context;
- current milestone id and status;
- workflow phase and next actions;
- summary counters for tasks;
- stage progress;
- gate summary.

User can:

- run `hlv_check`;
- run `hlv_task_sync`;
- refresh data.

Empty state rule:

- if there is no active milestone, show the workflow empty state and hide milestone-scoped sections such as Plan and Contracts instead of surfacing MCP errors as page-level failures.

### 5.3 Plan

Show stages in order and tasks inside each stage.

Data source rule:

- use `hlv://stage/{n}` or `hlv://projects/{id}/stage/{n}` for stage names, task names, dependencies, contracts, and output;
- use `hlv://tasks` only for current status/labels/meta summary;
- join tasks from `hlv://tasks` with tasks from `hlv://stage/{n}` by matching `task.id` (e.g. `TASK-001`);
- do not build the Plan screen from `hlv://tasks` alone because it does not include task names.

For each stage show:

- stage id;
- name;
- status.

For each task show:

- task id;
- name;
- status;
- labels;
- block reason if blocked.

Note on task IDs: task IDs are opaque strings parsed from stage markdown files. Common formats include `TASK-001` or `S1-001`; the dashboard must not assume a specific pattern.

Resource vs tool: `hlv://tasks` returns all tasks (no filtering); `hlv_task_list` supports filtering by `stage`, `status`, and `label`. Use the resource for full data loads and the tool for filtered views.

Optional for MVP, but useful:

- start task;
- mark task done;
- block/unblock task.

Guard rule:

- hide task actions when there is no active milestone.

### 5.4 Contracts / Gates

One simple tab or two separate tabs.

Contracts:

- id;
- formats.

Optional enrichment:

- if the UI needs `version` or `owner`, fetch `hlv://contracts/{id}` when the row is opened or lazily enrich rows after the list loads;
- do not require `status` for contracts list, because `hlv://contracts` does not provide it.

Gates:

- id;
- type;
- mandatory;
- enabled;
- cwd;
- command.

Optional gate actions:

- enable/disable;
- run gate.

### 5.5 Constraints

Show rule-based constraints (security, compliance, observability).

For each constraint file show:

- id;
- owner (if present);
- intent (if present);
- rule count.

For each rule show:

- rule id;
- severity (critical / high / medium / low);
- statement.

Optional constraint actions (beyond MVP):

- add/remove constraint;
- add/remove rule.

---

## 6. Realtime Behavior

The dashboard must use SSE subscriptions.

### 6.1 Subscribe to

Single-project mode:

- `hlv://milestones`
- `hlv://project`
- `hlv://gates`

Workspace mode:

- `hlv://projects/{id}/milestones`
- `hlv://projects/{id}/project`
- `hlv://projects/{id}/gates`

**Known quirk — workspace project subscription:**

The canonical resource for reading a project is `hlv://projects/{id}`, but the file watcher sends notifications on `hlv://projects/{id}/project`. These are different URIs.

In workspace mode you must:

1. subscribe to `hlv://projects/{id}/project` (this is what the watcher emits);
2. when the notification arrives, refetch `hlv://projects/{id}` (this is the read URI).

In single-project mode this quirk does not apply: both subscription and read use `hlv://project`.

### 6.2 Refetch rules

When `milestones` changes, refetch:

- milestones;
- tasks;
- workflow.

When `project` changes, refetch:

- project using `hlv://project` in single-project mode or `hlv://projects/{id}` in workspace mode.

When `gates` changes, refetch:

- gates.

Keep it simple:

- no complicated optimistic updates;
- after each successful mutation, just refetch the affected project data.

### 6.3 Watcher internals (for reference)

The server watches three files per project with a 100ms debounce:

| File | Notified resources |
|------|--------------------|
| `milestones.yaml` | milestones, tasks, workflow |
| `project.yaml` | project |
| `gates-policy.yaml` | gates |

No other files are watched. Stage markdown changes are invisible until `hlv_task_sync` is called.

---

## 7. Important Limitation

Current HLV MCP subscriptions do not watch:

- `human/milestones/*/stage_*.md`

Therefore the dashboard must not claim full realtime visibility for raw stage markdown edits until they are synced into HLV state.

Practical rule:

- if the UI depends on stage/task structure from stage files, provide a visible `Sync tasks` button using `hlv_task_sync`;
- after sync, refetch milestones/tasks/workflow.
- if there is no active milestone, hide or disable `Sync tasks`.

---

## 8. MCP Tool Catalog

All tools accept an optional `project_id` parameter. In workspace mode it is required; in single-project mode it is ignored.

### 8.1 MVP tools (minimum)

| Tool | Parameters | Description |
|------|-----------|-------------|
| `hlv_check` | — | Run validation checks, return diagnostics |
| `hlv_task_sync` | `force?: bool` | Sync tasks from stage files into milestones.yaml |
| `hlv_task_list` | `stage?: u32`, `status?: string`, `label?: string` | List tasks with optional filters |
| `hlv_workflow` | — | Get current phase, stages, next actions |

### 8.2 Task action tools

| Tool | Parameters | Description |
|------|-----------|-------------|
| `hlv_task_start` | `task_id: string` | Start a task → `in_progress` |
| `hlv_task_done` | `task_id: string` | Mark task → `done` |
| `hlv_task_block` | `task_id: string`, `reason: string` | Block a task with reason |
| `hlv_task_unblock` | `task_id: string` | Unblock a task |
| `hlv_task_label` | `task_id: string`, `action: "add"\|"remove"`, `label: string` | Add/remove task label |
| `hlv_task_meta` | `task_id: string`, `action: "set"\|"delete"`, `key: string`, `value?: string` | Set/delete task metadata |

### 8.3 Gate action tools

| Tool | Parameters | Description |
|------|-----------|-------------|
| `hlv_gate_enable` | `id: string` | Enable a gate |
| `hlv_gate_disable` | `id: string` | Disable a gate |
| `hlv_gate_run` | `id?: string` | Run one gate or all if omitted |

### 8.4 Milestone tools

| Tool | Parameters | Description |
|------|-----------|-------------|
| `hlv_milestone_new` | `name: string` | Create new milestone |
| `hlv_milestone_done` | — | Complete current milestone |
| `hlv_milestone_abort` | — | Abort current milestone |
| `hlv_milestone_label` | `action: "add"\|"remove"`, `label: string` | Label on milestone |
| `hlv_milestone_meta` | `action: "set"\|"delete"`, `key: string`, `value?: string` | Metadata on milestone |

### 8.5 Stage tools

| Tool | Parameters | Description |
|------|-----------|-------------|
| `hlv_stage_label` | `stage_id: u32`, `action: "add"\|"remove"`, `label: string` | Label on stage |
| `hlv_stage_meta` | `stage_id: u32`, `action: "set"\|"delete"`, `key: string`, `value?: string` | Metadata on stage |

### 8.6 Constraint tools (beyond MVP)

| Tool | Parameters | Description |
|------|-----------|-------------|
| `hlv_constraint_add` | `name: string`, `applies_to: string`, `owner?: string`, `intent?: string` | Add constraint file |
| `hlv_constraint_remove` | `name: string` | Remove constraint file |
| `hlv_constraint_add_rule` | `constraint: string`, `rule_id: string`, `severity: string`, `statement: string` | Add rule |
| `hlv_constraint_remove_rule` | `constraint: string`, `rule_id: string` | Remove rule |

### 8.7 Other tools

| Tool | Parameters | Description |
|------|-----------|-------------|
| `hlv_commit_msg` | `stage_complete?: bool`, `type?: string` | Generate commit message |
| `hlv_artifacts` | `scope?: "global"\|"milestone"`, `name?: string` | List/show artifacts |
| `hlv_glossary` | — | Show domain glossary |

### 8.8 Recommended read flow for project page

1. read project;
2. read milestones;
3. read workflow;
4. read gates;
5. if there is an active milestone, read tasks;
6. if there is an active milestone, read contracts;
7. if the Plan screen is opened and there is an active milestone, read `stage/{n}`.

If the dashboard is shipped read-only, that is acceptable for MVP.

---

## 9. UX Rules

Keep the UI operational, not decorative.

Required:

- clear status colors for `pending`, `in_progress`, `done`, `blocked`;
- visible connection state: connected / reconnecting / disconnected;
- visible loading and error states;
- manual refresh button;
- obvious stale note after connection loss.

Avoid:

- heavy kanban mechanics;
- drag-and-drop planning;
- background auto-mutations;
- custom workflow semantics;
- complex local caching logic.

---

## 10. Error Handling

If MCP is unavailable:

- show the last successful state if any;
- mark the UI as disconnected;
- retry connection.

If one widget fails:

- keep the rest of the page usable;
- show an inline error in that widget.

If a tool action fails:

- show the HLV error text;
- do not hide it behind generic "something went wrong".

---

## 11. Suggested Tech Stack

Not mandatory, just a practical default:

- React + TypeScript
- one MCP client layer
- one app store/query layer
- SSE subscription handling

No extra backend in MVP.

---

## 12. MVP Checklist

- [ ] Connect to `hlv mcp` over SSE
- [ ] Support single-project mode
- [ ] Support workspace mode
- [ ] Render workspace page
- [ ] Render project overview page
- [ ] Render plan page
- [ ] Render contracts/gates page
- [ ] Render constraints page
- [ ] Subscribe to updates
- [ ] Refetch on notifications
- [ ] Add manual refresh
- [ ] Add `Run Check`
- [ ] Add `Sync Tasks`
- [ ] Clearly show disconnected/stale state
- [ ] Optionally add task actions
- [ ] Optionally add gate actions

---

## 13. Summary

`SPEC_DASHBOARD` is intentionally small:

- direct browser -> `hlv mcp --transport sse`;
- workspace overview + project detail;
- realtime refresh via MCP subscriptions;
- no extra backend, no extra database, no invented state.

Build the simplest useful web UI over the HLV state that already exists.

---

## Appendix A. JSON Response Examples

### A.1 ProjectSummary (`hlv://projects`)

```json
[
  {
    "id": "backend",
    "root": "/home/user/projects/backend",
    "name": "Backend API",
    "current_milestone": "001",
    "milestone_status": "Implementing",
    "stages_total": 5,
    "stages_done": 2
  }
]
```

### A.2 MilestoneMap (`hlv://milestones`)

```json
{
  "project": "ecommerce-api",
  "current": {
    "id": "001",
    "number": 1,
    "branch": "feat/001-foundation",
    "stage": 1,
    "stages": [
      {
        "id": 1,
        "scope": "Foundation",
        "status": "implementing",
        "tasks": [
          {
            "id": "TASK-001",
            "status": "done",
            "started_at": "2026-03-08T10:00:00Z",
            "completed_at": "2026-03-08T11:30:00Z",
            "labels": ["backend"]
          },
          {
            "id": "TASK-002",
            "status": "in_progress",
            "started_at": "2026-03-08T11:30:00Z"
          }
        ]
      },
      {
        "id": 2,
        "scope": "API Features",
        "status": "pending"
      }
    ],
    "labels": ["v1"],
    "meta": {"owner": "backend-team"}
  },
  "history": []
}
```

Notes: `tasks`, `labels`, `meta`, `gate_results` are omitted from JSON when empty.

### A.3 WorkflowData (`hlv://workflow`)

```json
{
  "milestone_id": "001",
  "phase": 4,
  "phase_name": "Implement",
  "stages": [
    {
      "id": 1,
      "scope": "Foundation",
      "status": "implementing",
      "active": true,
      "task_count": 3,
      "tasks_done": 2
    },
    {
      "id": 2,
      "scope": "API Features",
      "status": "pending",
      "active": false,
      "task_count": 4,
      "tasks_done": 0
    }
  ],
  "next_actions": [
    "Complete remaining tasks in Stage 1",
    "Review contracts before implementing Stage 2"
  ]
}
```

### A.4 TaskView (`hlv://tasks`)

```json
[
  {
    "stage_id": 1,
    "id": "TASK-001",
    "status": "in_progress",
    "started_at": "2026-03-08T10:00:00Z",
    "labels": ["frontend", "critical"],
    "meta": {"priority": "high"}
  },
  {
    "stage_id": 1,
    "id": "TASK-002",
    "status": "blocked",
    "block_reason": "waiting for API access",
    "started_at": "2026-03-08T09:30:00Z"
  }
]
```

Notes: `started_at`, `completed_at`, `block_reason`, `labels`, `meta` are omitted when empty/null.

### A.5 StagePlan (`hlv://stage/{n}`)

```json
{
  "id": 1,
  "name": "Foundation",
  "budget": "25K",
  "contracts": ["order.create", "order.cancel"],
  "tasks": [
    {
      "id": "TASK-001",
      "name": "Domain Types & Glossary",
      "contracts": ["order.create"],
      "depends_on": [],
      "output": ["llm/src/domain/"]
    },
    {
      "id": "TASK-002",
      "name": "order.create handler",
      "contracts": ["order.create"],
      "depends_on": ["TASK-001"],
      "output": ["llm/src/features/order_create/"]
    }
  ],
  "remediation": []
}
```

### A.6 Project (`hlv://project`)

```json
{
  "name": "payments",
  "version": "0.1.0",
  "methodology": "hlv",
  "paths": {
    "human": "human",
    "output": "output"
  }
}
```

### A.7 Contract detail (`hlv://contracts/{id}`)

The response includes `id`, `formats`, and one key per available format with the parsed contract object.

```json
{
  "id": "order.create",
  "formats": ["markdown", "yaml"],
  "markdown": {
    "id": "order.create",
    "version": "1.0",
    "owner": "backend-team",
    "sources": ["PRD §3.1"],
    "intent": "Create a new order from cart contents",
    "input_yaml": "customer_id: uuid\nitems: list<OrderItem>",
    "output_yaml": "order_id: uuid\nstatus: string",
    "errors": [
      {"code": "ORD-001", "trigger": "empty cart", "action": "reject"}
    ],
    "invariants": ["order total > 0"],
    "examples": [],
    "edge_cases": [],
    "security": [],
    "sections": []
  },
  "yaml": {
    "id": "order.create",
    "version": "1.0",
    "owner": "backend-team",
    "intent": "Create a new order from cart contents",
    "errors": [],
    "invariants": []
  }
}
```

Notes: `markdown` and `yaml` keys are present only if the corresponding file exists. Typically a contract has one or both formats.

### A.8 Contracts list (`hlv://contracts`)

```json
[
  {
    "id": "order.create",
    "formats": ["markdown", "yaml"]
  },
  {
    "id": "order.cancel",
    "formats": ["markdown"]
  }
]
```

### A.9 GatesPolicy (`hlv://gates`)

```json
{
  "version": "1.0",
  "policy_id": "gate-policy-v1",
  "description": "Release validation gates",
  "gates": [
    {
      "id": "GATE-001",
      "type": "unit-tests",
      "mandatory": true,
      "enabled": true,
      "command": "cargo test",
      "cwd": "llm"
    },
    {
      "id": "GATE-002",
      "type": "contract-validation",
      "mandatory": true,
      "enabled": true,
      "command": "hlv verify"
    }
  ]
}
```

### A.10 ConstraintFile (`hlv://constraints`)

```json
[
  {
    "id": "security-constraints",
    "version": "1.0",
    "owner": "security-team",
    "intent": "Enforce security best practices",
    "rules": [
      {
        "id": "SEC-001",
        "severity": "critical",
        "statement": "All endpoints must use HTTPS",
        "enforcement": ["linter", "runtime-check"]
      }
    ]
  }
]
```

### A.11 ArtifactIndex (`hlv://artifacts`)

```json
{
  "global": [
    {"name": "context", "path": "human/artifacts/context.md", "kind": "context"},
    {"name": "stack", "path": "human/artifacts/stack.md", "kind": "stack"}
  ],
  "milestone": [
    {"name": "feature-auth", "path": "human/milestones/001/artifacts/feature-auth.md", "kind": "feature"}
  ]
}
```

### A.12 Glossary (`hlv://glossary`)

```json
{
  "schema_version": 1,
  "domain": "ecommerce",
  "terms": [
    {"term": "Order", "definition": "A customer request to purchase items", "context": "domain"}
  ],
  "types": {
    "OrderId": {"base": "uuid", "description": "Unique order identifier"},
    "Money": {"base": "decimal", "description": "Monetary amount in cents"}
  },
  "enums": {
    "OrderStatus": {
      "variants": ["pending", "confirmed", "shipped", "cancelled"],
      "description": "Lifecycle states of an order"
    }
  }
}
```

Notes: `terms`, `types`, `enums` are omitted from JSON when empty. The glossary is served as JSON (not raw YAML).

### A.13 LlmMap (`hlv://map`)

```json
{
  "schema_version": 1,
  "ignore": ["target/**", "*.log"],
  "entries": [
    {"path": "llm/src", "kind": "dir", "layer": "llm", "description": "Generated source code"},
    {"path": "human/artifacts", "kind": "dir", "layer": "human", "description": "Domain artifacts"}
  ]
}
```

---

## Appendix B. Tool Response Examples

Tool responses are wrapped in the standard MCP `tools/call` result format. The `text` field contains the tool-specific payload.

### B.1 `hlv_check`

```json
{
  "exit_code": 1,
  "diagnostics": [
    {"code": "PRJ-010", "severity": "error", "message": "project.yaml not found"},
    {"code": "MIL-020", "severity": "warning", "message": "No active milestone"}
  ]
}
```

`exit_code` is `0` when all checks pass, `1` when there are errors.

### B.2 `hlv_task_sync`

Returns a plain text message:

```
"Synced 5 tasks (2 new, 1 removed)"
```

### B.3 `hlv_task_start` / `hlv_task_done` / `hlv_task_block` / `hlv_task_unblock`

Returns a plain text confirmation:

```
"Task 'S2-001' started"
```

### B.4 `hlv_task_list`

Returns the same JSON format as `hlv://tasks` (array of task objects), filtered by the provided parameters.

### B.5 `hlv_workflow`

Returns the same JSON format as `hlv://workflow`.

### B.6 `hlv_gate_run`

Returns a plain text summary with per-gate pass/fail/skip results:

```
"GATE-001 (unit-tests): passed\nGATE-002 (contract-validation): failed\n\n1 passed, 1 failed, 0 skipped"
```

### B.7 `hlv_milestone_new`

Returns a plain text confirmation:

```
"Milestone '002-auth' created"
```

For full request/response wrapping examples, see `docs/MCP.md`.
