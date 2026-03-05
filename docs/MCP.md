# HLV MCP Server

MCP (Model Context Protocol) server for HLV. Provides programmatic access to HLV project data and operations.

## Launch

```bash
# stdio transport (for Claude Code and CLI clients)
hlv mcp

# SSE/HTTP transport (for web clients)
hlv mcp --transport sse --port 3000

# Workspace (multi-project) - several projects through one server
hlv mcp --workspace ~/.hlv/workspace.yaml
hlv mcp --workspace ~/.hlv/workspace.yaml --transport sse --port 3000
```

## Claude Code Configuration

`.mcp.json` in the project root (single-project mode):

```json
{
  "mcpServers": {
    "hlv": {
      "command": "hlv",
      "args": ["mcp"],
      "cwd": "/path/to/project"
    }
  }
}
```

Workspace mode (multi-project):

```json
{
  "mcpServers": {
    "hlv-workspace": {
      "command": "hlv",
      "args": ["mcp", "--workspace", "/home/dev/.hlv/workspace.yaml"]
    }
  }
}
```

---

## Resources (read-only)

### Static resources

| Resource | URI | Description |
|---|---|---|
| Project | `hlv://project` | Project configuration (`project.yaml`) |
| Milestones | `hlv://milestones` | Current milestone, stages, history |
| Gates | `hlv://gates` | Gate policy (`gates-policy.yaml`) |
| Constraints | `hlv://constraints` | List of constraint files |
| Map | `hlv://map` | LLM file navigator (`map.yaml`) |
| Workflow | `hlv://workflow` | Current phase + recommended actions |
| Tasks | `hlv://tasks` | All tasks across all stages |
| Artifacts | `hlv://artifacts` | Global artifacts (metadata) |
| Contracts | `hlv://contracts` | List of contracts for the current milestone |
| Plan | `hlv://plan` | Plan structure (`plan.md`) |
| Traceability | `hlv://traceability` | Requirement <-> contract <-> code links |
| Glossary | `hlv://glossary` | Domain terms |

### Parameterized resources

| Resource | URI Template | Description |
|---|---|---|
| Stage | `hlv://stage/{n}` | Plan for a specific stage |
| Contract | `hlv://contracts/{id}` | Full contract content |
| Tasks (stage) | `hlv://tasks/{n}` | Tasks for a specific stage |
| Artifact | `hlv://artifacts/{name}` | Global artifact content |
| Milestone artifacts | `hlv://artifacts/milestone/{mid}` | Milestone artifacts (metadata) |
| Milestone artifact | `hlv://artifacts/milestone/{mid}/{name}` | Milestone artifact content |

### Response examples

**`hlv://project`**

```json
{
  "contents": [{
    "uri": "hlv://project",
    "mimeType": "application/json",
    "text": "{\"name\":\"payments\",\"version\":\"0.1.0\",\"methodology\":\"hlv\",\"paths\":{\"human\":\"human\",\"output\":\"output\"}}"
  }]
}
```

**`hlv://milestones`**

```json
{
  "contents": [{
    "uri": "hlv://milestones",
    "mimeType": "application/json",
    "text": "{\"current\":{\"id\":\"001-order-create\",\"name\":\"Order Create\",\"status\":\"implementing\",\"stages\":[{\"id\":1,\"status\":\"validated\"},{\"id\":2,\"status\":\"implementing\"}],\"labels\":[],\"meta\":{}},\"history\":[]}"
  }]
}
```

**`hlv://tasks` (all tasks)**

```json
{
  "contents": [{
    "uri": "hlv://tasks",
    "mimeType": "application/json",
    "text": "[{\"stage_id\":1,\"id\":\"S1-001\",\"status\":\"done\"},{\"stage_id\":2,\"id\":\"S2-001\",\"status\":\"in_progress\",\"labels\":[\"api\"],\"meta\":{\"owner\":\"team-a\"}}]"
  }]
}
```

**`hlv://stage/{n}` (parameterized)**

```json
{
  "contents": [{
    "uri": "hlv://stage/1",
    "mimeType": "application/json",
    "text": "{\"id\":1,\"name\":\"Core API\",\"budget\":\"4h\",\"contracts\":[\"C-001\",\"C-002\"],\"tasks\":[{\"id\":\"S1-001\",\"name\":\"Create order endpoint\",\"contracts\":[\"C-001\"],\"depends_on\":[],\"output\":[\"src/order.rs\"]}],\"remediation\":[]}"
  }]
}
```

**`hlv://glossary`**

```json
{
  "contents": [{
    "uri": "hlv://glossary",
    "mimeType": "application/json",
    "text": "{\"terms\":[{\"term\":\"Order\",\"definition\":\"A customer request to purchase items\",\"context\":\"domain\"}]}"
  }]
}
```

---

## Tools (operations)

### Core

| Tool | Parameters | Description |
|---|---|---|
| `hlv_check` | - | Validate the project and return diagnostics |
| `hlv_workflow` | - | Current phase + recommended actions |
| `hlv_commit_msg` | `stage_complete?`, `type?` | Generate a commit message |

### Milestone

| Tool | Parameters | Description |
|---|---|---|
| `hlv_milestone_new` | `name` | Create a new milestone |
| `hlv_milestone_done` | - | Finish the current milestone |
| `hlv_milestone_abort` | - | Abort the current milestone |
| `hlv_milestone_label` | `action` (add/remove), `label` | Manage labels |
| `hlv_milestone_meta` | `action` (set/delete), `key`, `value?` | Manage metadata |

### Gates

| Tool | Parameters | Description |
|---|---|---|
| `hlv_gate_enable` | `id` | Enable a gate |
| `hlv_gate_disable` | `id` | Disable a gate |
| `hlv_gate_run` | `id?` | Run gate(s) |

### Constraints

| Tool | Parameters | Description |
|---|---|---|
| `hlv_constraint_add` | `name`, `owner?`, `intent?`, `applies_to` | Add a constraint |
| `hlv_constraint_remove` | `name` | Remove a constraint |
| `hlv_constraint_add_rule` | `constraint`, `rule_id`, `severity`, `statement` | Add a rule |
| `hlv_constraint_remove_rule` | `constraint`, `rule_id` | Remove a rule |

### Stages

| Tool | Parameters | Description |
|---|---|---|
| `hlv_stage_reopen` | `stage_id` | Reopen a stage (implemented→implementing, validated→validating) |
| `hlv_stage_label` | `stage_id`, `action`, `label` | Add/remove a stage label |
| `hlv_stage_meta` | `stage_id`, `action`, `key`, `value?` | Stage metadata |

### Tasks

| Tool | Parameters | Description |
|---|---|---|
| `hlv_task_list` | `stage?`, `status?`, `label?` | List tasks with filters |
| `hlv_task_add` | `stage_id`, `task_id`, `name` | Add a task (auto-reopens stage if needed) |
| `hlv_task_start` | `task_id` | Start a task |
| `hlv_task_done` | `task_id` | Complete a task |
| `hlv_task_block` | `task_id`, `reason` | Block a task |
| `hlv_task_unblock` | `task_id` | Unblock a task |
| `hlv_task_sync` | `force?` | Sync tasks from stage files |
| `hlv_task_label` | `task_id`, `action`, `label` | Add/remove a task label |
| `hlv_task_meta` | `task_id`, `action`, `key`, `value?` | Task metadata |

### Artifacts & Glossary

| Tool | Parameters | Description |
|---|---|---|
| `hlv_artifacts` | `scope?`, `name?` | List/show artifacts |
| `hlv_glossary` | - | Domain glossary |

### Request/response examples

**`hlv_task_list`** - request:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "hlv_task_list",
    "arguments": { "status": "in_progress" }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [{
      "type": "text",
      "text": "[{\"stage_id\":2,\"id\":\"S2-001\",\"status\":\"in_progress\"}]"
    }]
  }
}
```

**`hlv_check`** - request:

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": { "name": "hlv_check", "arguments": {} }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [{
      "type": "text",
      "text": "{\"exit_code\":1,\"diagnostics\":[{\"code\":\"PRJ-010\",\"severity\":\"error\",\"message\":\"project.yaml not found\"}]}"
    }]
  }
}
```

**`hlv_task_start`** - request:

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "hlv_task_start",
    "arguments": { "task_id": "S2-001" }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [{
      "type": "text",
      "text": "Task 'S2-001' started"
    }]
  }
}
```

---

## Notifications

> **SSE mode only.** Change subscriptions (`resources/subscribe`) are available only when launched with `--transport sse`. In stdio mode, the file watcher is not started and the `resources.subscribe` capability is not advertised.

### Watched files

| File | Resources updated on change |
|---|---|
| `milestones.yaml` | `hlv://milestones`, `hlv://tasks`, `hlv://workflow` |
| `project.yaml` | `hlv://project` |
| `gates-policy.yaml` | `hlv://gates` |

### Subscription

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "resources/subscribe",
  "params": { "uri": "hlv://milestones" }
}
```

When the file changes, the client receives a notification:

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/resources/updated",
  "params": { "uri": "hlv://milestones" }
}
```

---

## Transports

### stdio (default)

JSON-RPC 2.0 over stdin/stdout. Used for Claude Code and CLI integrations.

```bash
hlv mcp
```

### SSE (Streamable HTTP)

HTTP server using the MCP Streamable HTTP transport. For web clients and dashboards.

```bash
hlv mcp --transport sse --port 3000
```

- Endpoint: `POST /mcp` — JSON-RPC requests; responses are `text/event-stream`
- Endpoint: `GET /mcp` — reconnect to an existing SSE session (requires `Mcp-Session-Id`)
- Endpoint: `DELETE /mcp` — close session
- CORS: all origins allowed (for browser clients)
- Graceful shutdown: `SIGINT`/`SIGTERM`

**Key transport details:**

- **Accept header is required.** Every request must include `Accept: application/json, text/event-stream`. The server returns `406 Not Acceptable` without it.
- **All responses are SSE.** POST responses arrive as `text/event-stream`, not plain JSON. Each JSON-RPC message is a `data:` line followed by a blank line.
- **POST initialize is the session entry point.** The first POST with `initialize` creates a session and returns a long-lived SSE stream. The `Mcp-Session-Id` header in the response identifies the session. This stream stays open and carries server-pushed notifications (e.g. `notifications/resources/updated`).
- **Session lifetime = stream lifetime.** If the SSE stream from `initialize` is closed, the session is destroyed and subsequent requests return `404 Session not found`. Use `GET /mcp` with `Mcp-Session-Id` to reconnect before the session expires.

### Connection lifecycle

1. `POST /mcp` — send `initialize`; keep the response stream open (it is the notification channel); read `Mcp-Session-Id` from the response header;
2. `POST /mcp` — send `notifications/initialized` (include `Mcp-Session-Id`);
3. `POST /mcp` — send `resources/subscribe` for each URI;
4. Notifications arrive on the SSE stream from step 1;
5. `DELETE /mcp` — close session when done.

### Parsing SSE responses

All POST responses (including tool calls and resource reads) arrive as SSE:

```
data: {"jsonrpc":"2.0","id":1,"result":{...}}

```

You cannot use `response.json()`. Parse SSE frames to extract the `data:` payload, then parse that as JSON.

### Connection examples

**JavaScript (SSE):**

```javascript
// Step 1: initialize — this response is a long-lived SSE stream
const response = await fetch('http://localhost:3000/mcp', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Accept': 'application/json, text/event-stream',
  },
  body: JSON.stringify({
    jsonrpc: '2.0',
    id: 1,
    method: 'initialize',
    params: {
      protocolVersion: '2025-03-26',
      capabilities: {},
      clientInfo: { name: 'my-client', version: '0.1.0' },
    },
  }),
});

// Read session ID from response header
const sessionId = response.headers.get('Mcp-Session-Id');

// Parse SSE stream for initialize response + future notifications
const reader = response.body.getReader();
const decoder = new TextDecoder();
// ... read and parse SSE frames from the stream

// Step 2: complete handshake (subsequent requests use sessionId)
await fetch('http://localhost:3000/mcp', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Accept': 'application/json, text/event-stream',
    'Mcp-Session-Id': sessionId,
  },
  body: JSON.stringify({
    jsonrpc: '2.0',
    method: 'notifications/initialized',
  }),
});
```

**Python (stdio):**

```python
import subprocess, json

proc = subprocess.Popen(
    ['hlv', 'mcp'],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE,
    cwd='/path/to/project',
)

request = json.dumps({
    "jsonrpc": "2.0", "id": 1,
    "method": "initialize",
    "params": {
        "protocolVersion": "2025-03-26",
        "capabilities": {},
        "clientInfo": {"name": "py-client", "version": "0.1"},
    },
}) + "\n"

proc.stdin.write(request.encode())
proc.stdin.flush()
```

**Rust (`rmcp` client):**

```rust
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;

let config = StreamableHttpClientTransportConfig::with_uri("http://localhost:3000/mcp");
let transport = config.start_client().await?;
let client = ().serve(transport).await?;
```

---

## Workspace (multi-project)

Workspace mode lets one MCP server serve multiple HLV projects.

### Configuration

```yaml
# ~/.hlv/workspace.yaml
projects:
  - id: backend
    root: /home/dev/projects/backend
  - id: mobile
    root: /home/dev/projects/mobile-app
  - id: infra
    root: /home/dev/projects/infra
```

Each project must contain `project.yaml` at the specified `root`.

### URI scheme

In workspace mode, all resources are available via `hlv://projects/{id}/...`:

| Resource | URI (single) | URI (workspace) |
|---|---|---|
| Project list | - | `hlv://projects` |
| Project | `hlv://project` | `hlv://projects/{id}` |
| Milestones | `hlv://milestones` | `hlv://projects/{id}/milestones` |
| Tasks | `hlv://tasks` | `hlv://projects/{id}/tasks` |
| Stage | `hlv://stage/{n}` | `hlv://projects/{id}/stage/{n}` |
| ... | `hlv://{resource}` | `hlv://projects/{id}/{resource}` |

### `hlv://projects` - overview of all projects

```json
[
  {
    "id": "backend",
    "root": "/home/dev/projects/backend",
    "name": "payments-api",
    "current_milestone": "001-order-create",
    "milestone_status": "Implementing",
    "stages_total": 3,
    "stages_done": 1
  },
  {
    "id": "mobile",
    "root": "/home/dev/projects/mobile-app",
    "name": "mobile-app",
    "current_milestone": "002-auth",
    "milestone_status": "Pending",
    "stages_total": 2,
    "stages_done": 0
  }
]
```

### Tools in workspace mode

All tools accept an additional `project_id` parameter:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "hlv_task_list",
    "arguments": {
      "project_id": "backend",
      "status": "in_progress"
    }
  }
}
```

In single-project mode `project_id` is ignored and can be omitted.
In workspace mode `project_id` is required; omitting it is an error.

### Use cases

- **Kanban across all projects**: `hlv://projects` -> iterate over `hlv://projects/{id}/tasks`
- **Overview dashboard**: status of each project, aggregate progress
- **AI assistant**: switches between projects based on context
- **Custom CLI/TUI**: aggregated view across all projects

### Notifications in workspace mode

When using SSE transport, a file watcher is started for each project in the workspace.
Notifications work the same way: when `milestones.yaml` changes in project `backend`,
the server sends `resources/updated` for `hlv://projects/backend/milestones`
(and related scoped URIs such as `hlv://projects/backend/tasks`, `hlv://projects/backend/workflow`).
