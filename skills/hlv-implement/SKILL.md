---
name: hlv-implement
description: Execute the implementation plan by assigning agents to tasks, generating code and tests from contracts. Agents work in parallel within stages. Use after /hlv-verify passes, when the user says "implement", "generate code", or "execute plan".
disable-model-invocation: true
allowed-tools: Read Write Edit Glob Grep Bash Agent
metadata:
  author: hlv
  version: "1.0"
---

# HLV Implement — Plan to Code + Tests

Execute the implementation plan: agents perform tasks from milestone stage files in parallel, generating code and tests from contracts.

## Step 0: Read Configuration

Before proceeding, read `project.yaml → features` and note the flag values:
- `features.linear_architecture` (default: `true`)
- `features.hlv_markers` (default: `true`)
- `features.security_markers` (default: `true`)
- `features.legacy_mode` (default: `false`)
- `features.index_tracking` (default: `ignored`)

These flags control which sections below are active. If `project.yaml` has no `features` section, use defaults: the first three booleans are `true`, `legacy_mode` is `false`, and `index_tracking` is `ignored`.
If `features.legacy_mode` is `true`, also read `paths.code` and query the signature index with `hlv index show --json <symbol>` or `hlv index list --json --file <path>` before changing observed legacy code. Legacy code stays in place; only new/changed milestone work must follow the full contract and marker flow.

## HLV Root Resolution

Before reading or reporting missing HLV files, resolve the project layout:

1. If `project.yaml` exists in the current project root, use greenfield layout: `CONFIG_ROOT = .`, `REPO_ROOT = .`.
2. Else if `.hlv/project.yaml` exists, use adopted layout: `CONFIG_ROOT = .hlv`, `REPO_ROOT = .`.
3. Else search upward for either `project.yaml` or `.hlv/project.yaml`.
4. Read `CONFIG_ROOT/project.yaml` first. HLV-owned paths such as `human/`, `validation/`, `llm/`, and `milestones.yaml` are relative to `CONFIG_ROOT`.
5. In the steps below, bare paths like `milestones.yaml` or `human/` mean `CONFIG_ROOT/milestones.yaml` and `CONFIG_ROOT/human/`.
6. In adopted projects, existing source/test roots from `paths.code` are relative to `REPO_ROOT`.

Never report that root-level `human/`, `validation/`, `milestones.yaml`, or `project.yaml` are missing until `.hlv/project.yaml` has been checked. Use `hlv check --root <REPO_ROOT>` for deterministic validation.

### Adopt Mode

When `legacy_mode` is enabled, treat `paths.code` as observed brownfield code and `.hlv/llm/` as HLV metadata. Do not relocate legacy files into `llm/src` or `.hlv/llm/src`. Before editing a legacy file, use the signature index to identify the relevant symbols, then apply the normal contract-driven workflow to the changed files.

## CRITICAL: Code Architecture Philosophy

> **Conditional: `features.linear_architecture: true`**
> If `linear_architecture` is `false` in project.yaml, skip this entire section. Use your preferred architecture style instead.

> **The human DOES NOT read the generated code. The code is written FOR machines — LLM agents read it, LLM agents modify it, automated gates validate it.**

This changes everything about how code is structured:

1. **Comments are for LLM navigation.** The contract IS the documentation. Comments in code are navigation markers for LLM, not explanations for humans. Format: `// @ctx: stock validation for order.create contract`. If something needs explaining — the contract is incomplete, go fix it.

2. **Maximize LLM-readability. No layered architecture.** Flat module structure. Explicit types everywhere. No "clever" patterns, no metaprogramming, no implicit behavior. **No layered architecture** — controller/service/repository is a pattern for humans. LLM writes linearly: input → validation → logic → output → errors. One file, one flow. An LLM agent with a 200K context window must understand any module in isolation.

3. **One contract = one module boundary.** Each contract maps to exactly one directory. All code for `order.create` lives in one directory. No cross-contract imports except through domain types.

4. **Domain types are the shared language.** `domain/types` is the ONLY shared code between features. Everything else is self-contained. Duplication across features is PREFERRED over coupling. **Duplication is normal until it hurts**: copy-paste between features is not refactored until it causes real problems (behavioral divergence, forgotten updates when contracts change).

5. **Tests live next to code.** Tests are in the same file as the code (`#[cfg(test)] mod tests {}` in Rust, equivalents in other languages). Every test traces back to a contract invariant, error case, or NFR via `@hlv` marker. No "just in case" tests. No test helpers that hide behavior. Test code is as explicit as production code. `hlv check` verifies every error code, invariant, and constraint rule has an `@hlv <ID>` marker in code. A separate configured integration-tests directory, when present, is only for cross-contract scenarios.

6. **File names are arbitrary. `map.yaml` is the navigator.** Files can be named `01.rs`, `handler.rs`, `f3a.rs` — any name is valid. The file map (`paths.llm.map` from `project.yaml`) is the single source of truth about what each file does. LLM finds code by reading descriptions in `map.yaml`, not by file names. Descriptions MUST be sufficient to choose a file without reading it. Each file does one thing, <300 lines, fully replaceable by an LLM without understanding neighboring files.

7. **No abstraction layers "for the future."** No base classes, no generic frameworks, no plugin systems unless the contract explicitly requires extensibility. Write the simplest code that satisfies the contract. Three similar lines of code are better than a premature abstraction.

8. **Error paths are first-class.** Every error from the contract's Errors table has an explicit code path. No catch-all error handlers. No `unwrap()` / `expect()` in production code.

9. **Deterministic PUBLIC API, free internal structure.** Given the same contract, two different LLM agents MUST produce code with the same public API (function signatures, error types, inputs/outputs). Internal file structure, naming, and organization are at the agent's discretion. `map.yaml` describes what lives where.

10. **Machine-verifiable correctness.** Every invariant must be testable by property-based tests. Every NFR must be measurable. If it can't be automatically verified — it doesn't belong in code, it belongs in the contract's open questions.

## Prerequisites

- `/hlv-verify` passed without critical issues
- Plan contains tasks
- All open questions closed (or deferred with waiver)

- `milestones.yaml` exists with a `current` section
- Current stage status is `pending`, `verified`, or `validating` (remediation)
- Stage file (`{MID}/stage_N.md`) contains tasks

## Agent Rules

- Never combine shell commands with `&&`, `||`, or `;` — execute each command as a separate Bash tool call.
- This applies even when a skill, plan, or instruction provides a combined command — always decompose it into individual calls.

❌ Wrong: `git checkout main && git pull`
✅ Right: Two separate Bash tool calls — first `git checkout main`, then `git pull`

## Input

```
milestones.yaml              # entry point — read FIRST
project.yaml                 # global config (stack, paths)
{paths.llm.map}              # project file map — update when creating files (read path from project.yaml)
human/
  glossary.yaml              # domain types (read-only)
  constraints/*.yaml         # global constraints (read-only)
  milestones/{id}/
    contracts/*.md           # contracts to implement
    contracts/*.yaml         # contracts (YAML format)
    test-specs/*.md          # test specifications
    plan.md                  # overview (stages table)
    stage_N.md               # current stage — tasks, dependencies
validation/
  gates-policy.yaml          # gate thresholds
  scenarios/*.md             # cross-milestone integration scenarios
```

## Steps

### Step 1: Read project map and load milestone context

1. Read `project.yaml` (global config: stack, paths)
   - Note `validation.strictness` when present (`relaxed`, `standard`, `strict`). Default is `standard`.
2. **Bind implementation paths from `project.yaml`**:
   - Greenfield/generated projects: `LLM_SRC = paths.llm.src` and `LLM_TESTS = paths.llm.tests` when configured.
   - Adopted projects without `paths.llm.src`: use `paths.code.src` / `paths.code.tests` as the editable project code roots; do not create `.hlv/llm/src`.
   - `LLM_MAP  = paths.llm.map`   (e.g. `llm/map.yaml`)
   **All subsequent steps MUST use these variables. Never assume `llm/src/` — always use the configured project paths.**

   > **HARD CONSTRAINT — Output directory isolation**
   > In greenfield mode, generated code and tests MUST stay inside configured `paths.llm` roots.
   > In adopt mode, existing project code roots from `paths.code` are editable; `.hlv/llm/` is metadata, not source code.
   > `hlv check` enforces this mechanically with `MAP-080` for implementation paths outside `LLM_SRC` and `MAP-081` for test paths outside `LLM_TESTS`.

3. Read `milestones.yaml` → get `current.id` and `current.stage` (current stage number)
4. Set `MID = human/milestones/{current.id}`
5. Find the current stage in `current.stages[]` by matching the stage number
6. **STATUS GATE (hard stop)**:
   - Read stage `status`
   - Allowed values to proceed: `pending`, `verified`, `implementing`, `validating`
   - `pending` — implementation without prior /hlv-verify
   - `verified` — normal implementation after /hlv-verify passed
   - `implementing` — re-run, continue from pending tasks
   - `validating` — remediation: /hlv-validate found gate failures and added FIX tasks to stage_N.md Remediation section. Execute only pending remediation tasks.
   - `implemented` or `validated` — this stage is done. Check if there's a next stage to advance to, or inform user.
7. Update stage status → `implementing` in `milestones.yaml` (schema: `schema/milestones-schema.json`)
8. Read `{MID}/stage_N.md` — load tasks for the current stage
9. Read `project.yaml → stack.components` — understand target languages, frameworks
10. Read `project.yaml → artifact_graph.code_ownership` when present. New or changed implementation/test/doc paths must preserve ownership mappings and relation fields (`implements`, `verifies`, `documents`, `requires`) so `hlv artifacts impact` can route downstream review.
    - `code-*` ownership and `implements` paths must remain under `LLM_SRC`.
    - `tests-*` ownership and `verifies` paths must remain under `LLM_TESTS`.
11. For every new or changed file under an artifact ownership path, add file-level evidence markers for the relevant relation, e.g. `@hlv:artifact code-auth implements spec-auth`, `@hlv:artifact tests-auth verifies spec-auth`, or `@hlv:artifact docs-auth documents spec-auth`. Use the native comment syntax for the file type.

### Step 2: Execute tasks

`/hlv-implement` works on ONE stage at a time. The current stage is determined by `milestones.yaml → current.stage`.

Tasks within a stage execute based on their dependency graph (topological sort):
- Tasks without unresolved `depends_on` → execute in parallel
- Tasks with `depends_on` → wait for predecessors

```
stage = read {MID}/stage_N.md
ready_tasks = tasks with no pending depends_on

while ready_tasks not empty:
  for task in ready_tasks (parallel):
    1. Load context: contract from {MID}/contracts/, glossary, test spec from {MID}/test-specs/, dependency outputs
    2. Generate code + tests within declared output paths (`LLM_SRC`, `LLM_TESTS`)
    3. Run local checks: compile, lint, unit tests
    4. Mark task completed in stage_N.md

  ready_tasks = recalculate from remaining pending tasks

Boundary: git commit after all stage tasks completed
Update milestones.yaml: stage status → implemented
```

After completing a stage, inform the user: "Stage N complete. Run `/hlv-validate` to check gates, or `/hlv-implement` for the next stage."

### Step 3: Agent protocol

Each agent when executing a task:

1. **Read** `{MID}/stage_N.md` → find assigned task
2. **Check** `depends_on` → all dependencies completed
3. **Load context**:
   - Contract (from `task.contracts` — `{MID}/contracts/`)
   - Glossary (`human/glossary.yaml`)
   - Stack (`project.yaml → stack.components`) — target language, framework, dependencies
   - Test spec (`{MID}/test-specs/<contract>.md`)
   - Dependent code (output of previous tasks)
4. **Generate (linear, inline, TDD)** — create files inside the bound implementation/test roots. In greenfield this is `LLM_SRC`/`LLM_TESTS`; in adopt mode this may be the existing `paths.code` roots:
   - **Code structure** *(when `features.linear_architecture: true`)*: write linearly — input → validation → logic → output → errors. No layers (controller/service/repository). One file per logical unit. File names are arbitrary (e.g., `01.rs`, `create.rs`) — describe each file in `LLM_MAP`. *(When `false`: use your preferred architecture style — layered, hexagonal, etc.)*
   - **Tests inline**: unit tests go in the same file as code (`#[cfg(test)] mod tests`). Separate `LLM_TESTS` directory only for integration tests.
   - **`@ctx` comments**: add LLM navigation markers — `// @ctx: stock check for order.create`. Not human docs, but LLM orientation.
   - **Tests first**: write unit tests from contract test spec and property-based tests from invariants BEFORE implementation code. Tests must compile (with stubs/unimplemented markers) and clearly fail.
   - **Then implement**: write implementation code to make the failing tests pass. *(When `features.linear_architecture: true`)* No layered abstractions — write the simplest linear code.
   - **Then refine**: once tests are green, refactor if needed while keeping tests green. Duplication across features is OK — don't extract until it hurts.
   - **`@hlv` markers** *(when `features.hlv_markers: true`, MANDATORY)*: every test MUST carry an `@hlv <ID>` comment linking it to a contract validation or constraint. See "Code Traceability Markers" below. *(When `false`: skip `@hlv` markers entirely.)*
5. **Validate locally**:
   - `cargo check` / `npm run build` / equivalent
   - Unit tests pass
   - Lint is clean
6. **Update `LLM_MAP`** (schema: `schema/llm-map-schema.json`):
   - Add entries for every new file and directory created during this task
   - Each entry: `path`, `kind` (file/dir), `layer: llm`, `description` (what the file does)
   - Do NOT add build artifacts, caches, or generated files — they should be covered by `ignore` patterns
   - If your stack produces new artifact types not yet ignored, add a pattern to the `ignore` list (e.g., `__pycache__`, `*.pyc`, `node_modules`, `target/`)
   - `hlv check` validates all map entries exist — missing entries are errors; LLM implementation/test entries outside `LLM_SRC`/`LLM_TESTS` are `MAP-080`/`MAP-081` errors
7. **Update** `stage_N.md`:
   - `task.status → completed`
   - `task.agent → <agent_id>`

### Logging Protocol (mandatory for all agents)

Every agent MUST add structured logging to ALL generated code. This is not optional — observability is a first-class constraint.

**Stack-specific instrumentation:**

| Stack | Library | Entry/exit | Error | State change |
|-------|---------|-----------|-------|-------------|
| Rust | `tracing` | `#[instrument]` on every pub fn | `error!(error = %e, ctx = ?ctx)` | `info!(entity_id, old, new, "state changed")` |
| Python | `structlog` | `log.info("handler.enter", **params)` | `log.error("op.failed", error=str(e), ctx=ctx)` | `log.info("state.changed", entity=id, old=old, new=new)` |
| Node | `pino` | `log.info({ params }, 'handler.enter')` | `log.error({ err, ctx }, 'op.failed')` | `log.info({ entityId, old, new }, 'state.changed')` |

**Rules:**
1. **Structured only** — no `println!`, `dbg!`, bare `console.log`. All output through the logging library.
2. **Every pub fn gets a span** — `#[instrument]` (Rust) or equivalent. Includes function args (excluding sensitive data).
3. **Every error path logs** — with `request_id`, `entity_id`, input summary, and error details. No silent catches.
4. **Every state mutation logs** — entity ID, old state, new state. DB writes, status transitions, cache ops.
5. **Every external call logs** — target, duration, outcome. HTTP, DB, queue, gRPC.
6. **Request correlation** — propagate `request_id` / `trace_id` through all spans. Set at entry point, flows down.
7. **Sensitive data masked** — PII, tokens, passwords never appear in logs. Use `#[instrument(skip(password))]` or field redaction.
8. **Log levels correct** — `error` for failures, `warn` for degraded/retries, `info` for business events, `debug` for diagnostics.

**`@hlv` markers**: tests for logging rules use markers from `constraints/observability.yaml` (e.g., `@hlv structured_logging_only`, `@hlv log_all_errors`, `@hlv request_correlation`).

### Step 4: Coordination rules

1. **File isolation**: two agents NEVER write to the same file.
   - Task output paths do NOT overlap.
   - If overlap detected — block task, escalate to human.

2. **Shared read-only context**: agents READ shared files (glossary, contracts, domain types) but do NOT modify them.

3. **Stage boundary commit**: after all tasks in a stage complete — `git commit`.
   Artifacts become available to the next stage through git.

4. **Context budget**: each task has a `context_budget` in stage_N.md.
   If actual context (contract + glossary + deps) exceeds budget — split the task.

5. **Conflict resolution**: if two agents discover a conflict (both want to modify the same type) — block task, escalate to human.

### Step 5: Output summary

After all tasks in the current stage complete:

```
=== /hlv-implement complete (Stage N) ===

Milestone:           <milestone-id>
Stage:               <N>/<total>
Tasks completed:     <N>/<N>
Files generated:     <N>
Tests generated:     <N>

Next step: run /hlv-validate to check gates for this stage
```

### Step 6: Update project files

Update `milestones.yaml` (schema: `schema/milestones-schema.json`):

```yaml
# milestones.yaml updates:
current.stages[N].status: implementing → implemented
```

### Step 7: Set gate commands

After implementation, update `validation/gates-policy.yaml` (schema: `schema/gates-policy-schema.json`) — set the `command` field for each gate so that `hlv check` and `hlv gates run` can execute them automatically.

Determine the correct command from `project.yaml → stack` (language, framework):

| Gate type | Rust | Python | Node |
|-----------|------|--------|------|
| `contract_tests` | `cargo test --lib` | `pytest tests/contract/` | `npm test` |
| `integration_tests` | `cargo test --test integration` | `pytest tests/integration/` | `npm run test:integration` |
| `property_based_tests` | `cargo test --lib -- pbt` | `pytest tests/pbt/` | `npm run test:pbt` |
| `security` | `cargo audit` | `bandit -r src/` | `npm audit` |
| `mutation_testing` | `cargo mutants` | `mutmut run` | `npx stryker run` |
| `performance` | `cargo bench` | `locust --headless` | `npx k6 run` |

For each gate in `gates-policy.yaml`:
- If this stage produced test code covering this gate → set `command` and `cwd`, ensure `enabled: true`
- If the gate has no tests yet (will be covered in a later stage) → leave `command: null`
- Do NOT disable (`enabled: false`) gates that the user has enabled — only the user controls enable/disable

Also set the `cwd` field — the working directory relative to project root where the command should run. Derive this from `LLM_SRC` (e.g. if `paths.llm.src` is `llm/src/`, cwd is `llm`; if it's `apps/backend/src/`, cwd is `apps/backend`). Security gates may run from root.

Example update to `gates-policy.yaml`:
```yaml
gates:
  - id: GATE-CONTRACT-001
    type: contract_tests
    mandatory: true
    enabled: true
    command: "cargo test --lib"
    cwd: llm
    pass_criteria:
      required_scenarios_pass_rate: 1.0
  - id: GATE-SECURITY-001
    type: security
    mandatory: true
    enabled: true
    command: "cargo audit"
    cwd: llm
    pass_criteria:
      max_open_critical: 0
```

The user can also manage gates manually via CLI or dashboard (`hlv dashboard` → Gates tab):
- `hlv gates set-cmd <GATE-ID> "<command>"`
- `hlv gates set-cwd <GATE-ID> "<dir>"`
- `hlv gates clear-cmd/clear-cwd <GATE-ID>`
- `hlv gates enable/disable <GATE-ID>`

## Output

Generated code MUST go inside the paths configured in `project.yaml`:
- **Source code** → `LLM_SRC` when `paths.llm.src` exists, otherwise the selected `paths.code.src` root in adopt mode.
- **Integration tests** → `LLM_TESTS` when `paths.llm.tests` exists, otherwise the selected `paths.code.tests` root in adopt mode.
- **File map** → `LLM_MAP` (bound in Step 1 from `paths.llm.map`)

**Never hardcode `llm/src/` or `llm/tests/`** — always use the configured paths. In adopt mode, never create `.hlv/llm/src` for project source code.

Example layout when `paths.llm.src: llm/src/`, `paths.llm.tests: llm/tests/`, `paths.llm.map: llm/map.yaml`:

```
llm/
  src/                          # LLM_SRC — generated code (unit tests inline via #[cfg(test)])
    domain/types.rs             # from TASK-001 (types + tests in same file)
    domain/errors.rs
    features/order_create/      # from TASK-002 (handler + tests in same file)
    features/order_cancel/      # from TASK-003
    middleware/                  # from TASK-004
    observability/              # from TASK-006
  tests/                        # LLM_TESTS — integration tests ONLY (cross-contract scenarios)
    integration/                # from TASK-005
  map.yaml                      # LLM_MAP — updated with new entries

milestones.yaml                 # updated stage status
```

## Code Traceability Markers (`@hlv`)

> **Conditional: `features.hlv_markers: true`**
> If `hlv_markers` is `false` in project.yaml, skip this entire section. No `@hlv` markers are required and `hlv check` will not run CTR-010/CTR-001 checks.

Every contract validation and constraint rule MUST be traceable to test code. `hlv check` enforces this automatically.

### What gets tracked

| Source | Field | Example ID |
|--------|-------|------------|
| Contract errors | `errors[].code` | `OUT_OF_STOCK`, `INVALID_QUANTITY` |
| Contract invariants | `invariants[].id` | `atomicity`, `non_negative_total` |
| Constraint rules | `rules[].id` | `prepared_statements_only`, `no_secrets_in_logs` |

### Marker format

Add `@hlv <ID>` as a comment next to the test that verifies this validation:

```rust
// @ctx: stock validation for order.create contract
// @hlv OUT_OF_STOCK
#[test]
fn test_out_of_stock_returns_409() {
    // ...
}

// @ctx: transactional write — 3 tables in one tx
// @hlv atomicity
#[test]
fn test_order_write_is_atomic() {
    // ...
}

// @hlv prepared_statements_only
#[test]
fn test_no_sql_injection() {
    // ...
}
```

Works with any language — the marker is matched by text search, not syntax:

```python
# @ctx: user lookup in cancel flow
# @hlv USER_NOT_FOUND
def test_user_not_found():
    ...
```

```typescript
// @hlv pii_masking_enabled
it('masks PII in logs', () => { ... });
```

`@ctx` comments are optional LLM navigation markers — they help LLM orient quickly without reading the full file. Not human documentation.

### Rules

1. One `@hlv` marker per validation/constraint per test. A test may carry multiple markers if it covers several validations.
2. Every `errors[].code` from every contract YAML must appear as `@hlv <code>` somewhere in `LLM_SRC` or `LLM_TESTS`.
3. Every `invariants[].id` must appear as `@hlv <id>`.
4. Every constraint `rules[].id` must appear as `@hlv <id>` — except rules that have `check_command` (they are verified programmatically, not via markers).
5. `hlv check` reports missing markers as warnings (`CTR-010`). At `implemented` phase and later, these become hard warnings that block `/hlv-validate`. `hlv check` also runs `check_command` for rules that define one (CST-050/CST-060).

### Verification

```
$ hlv check
...
  Code traceability
    ! WRN [CTR-010] error 'OUT_OF_STOCK' from order.create has no @hlv marker in code
    ! WRN [CTR-010] constraint 'no_secrets_in_logs' from security.global has no @hlv marker in code
    · INF [CTR-001] Code traceability: 7/9 markers covered
```

## Security Attention Markers (`@hlv:sec`)

> **Conditional: `features.security_markers: true`**
> If `security_markers` is `false` in project.yaml, skip this entire section. No `@hlv:sec` markers are required and `hlv check` will not run SEC-010 diagnostics.

When writing implementation code, mark security-sensitive spots with `@hlv:sec` markers. These are attention flags for heightened scrutiny during `/hlv-validate`.

### Syntax

```
// @hlv:sec [CATEGORY] — free text reason
```

### Categories

| Category | When to use |
|----------|-------------|
| `INPUT_VALIDATION` | User input parsing, sanitization, boundary checks |
| `DESERIALIZATION` | Parsing external data (JSON, YAML, protobuf, etc.) |
| `AUTH_BOUNDARY` | Authentication/authorization checks, session validation |
| `SECRET_HANDLING` | API keys, tokens, passwords, PII in memory or logs |
| `FILE_ACCESS` | File reads/writes, path traversal risks |
| `CRYPTO` | Encryption, hashing, signing, random number generation |
| `PRIVILEGE_ESCALATION` | Role changes, sudo/admin operations, capability grants |
| `NETWORK` | HTTP requests, DNS, TLS, socket operations |

### Examples

```rust
// @hlv:sec [INPUT_VALIDATION] — user-supplied email used in DB query
fn create_user(email: &str) -> Result<User> { ... }

// @hlv:sec [SECRET_HANDLING] — API key loaded from env, must not leak to logs
let api_key = std::env::var("API_KEY")?;

// @hlv:sec [AUTH_BOUNDARY] — session token validated before granting access
fn verify_session(token: &str) -> Result<Session> { ... }
```

### Rules

1. Place `@hlv:sec` markers in **implementation code** (not just tests) at the point where the security-sensitive operation happens.
2. Use exactly one of the 8 categories above — `hlv check` warns on unknown categories (SEC-011).
3. Add a brief reason after `—` explaining why this spot is security-sensitive.
4. `hlv check` reports SEC-010 as Info: an aggregated summary table of markers by category and file count.

### Verification

```
$ hlv check
...
  Security markers
    · INF [SEC-010] Security markers: 5 total across 3 file(s) [AUTH_BOUNDARY=2, INPUT_VALIDATION=2, SECRET_HANDLING=1]
```

## Error handling

- Stage status not in allowed set (`pending`, `verified`, `implementing`, `validating`) → **hard stop** with guidance
- Open questions remain → error: "Resolve open questions before /hlv-implement"
- Task dependency cycle detected → error: "Dependency cycle in plan: \<details\>"
- File conflict between agents → block task, escalate to human
- Context budget exceeded → warning: "Task \<id\> exceeds context budget. Consider splitting."
- Local checks fail → retry once, then block task with error details

## Re-run

`/hlv-implement` can be run again:

1. Skips tasks with `status: completed`
2. Continues from first `pending` task
3. On contract change — marks affected tasks as `pending`

## Handoff integration

When a Handoff server is available:

1. `handoff_register` — register each agent
2. `handoff_check` — check for conflicts before writing a file
3. `handoff_done` — signal task completion
4. Change propagation — Handoff automatically notifies dependent agents

## Commit hint

After all tasks in a stage are done, check for `<!-- hlv:commit-hint -->` in the stage_N.md file. If present, suggest the user commit with the provided message:

```
git commit -m "$(hlv commit-msg)"
```

Or show the hint text and let the user decide.

## Cleanup

After the skill completes:
1. Run `hlv doctor` to catch missing paths, invalid command strings, cwd problems, schema mismatch, and non-ASCII rendering issues.
2. Run `hlv check` to validate the project structure. If there are errors — fix them before finishing. If `validation.strictness: strict` or CI parity is required, run `hlv check --strict`.
3. Suggest the user run `/clear` to free up context window before the next skill.
