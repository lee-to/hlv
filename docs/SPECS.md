# HLV Project File Specification

Description of all file contracts in the `human/`, `validation/`, and `llm/` directories, including their formats and purpose.

Example project: `tests/fixtures/example-project/`

JSON Schema for all YAML contracts: `schema/`

| Schema | YAML file | Rust model |
|--------|----------|-------------|
| `milestones-schema.json` | `milestones.yaml` | `model::milestone::MilestoneMap` |
| `project-schema.json` | `project.yaml` | `model::project::ProjectMap` |
| `glossary-schema.json` | `human/glossary.yaml` | `model::glossary::Glossary` |
| `contract-schema.json` | `human/milestones/{id}/contracts/*.yaml` | `model::contract_yaml::ContractYaml` |
| `constraint-schema.json` | `human/constraints/*.yaml` | `model::policy::ConstraintFile` |
| `security-constraints-schema.json` | `human/constraints/security.yaml` | `model::policy::SecurityConstraints` |
| `performance-constraints-schema.json` | `human/constraints/performance.yaml` | `model::policy::PerformanceConstraints` |
| `traceability-schema.json` | `human/traceability.yaml` | `model::traceability::TraceabilityMap` |
| `llm-map-schema.json` | `llm/map.yaml` | `model::llm_map::LlmMap` |
| `gates-policy-schema.json` | `validation/gates-policy.yaml` | `model::policy::GatesPolicy` |
| `equivalence-policy-schema.json` | `validation/equivalence-policy.yaml` | `model::policy::EquivalencePolicy` |
| `traceability-policy-schema.json` | `validation/traceability-policy.yaml` | `model::policy::TraceabilityPolicy` |
| `ir-policy-schema.json` | `validation/ir-policy.yaml` | `model::policy::IrPolicy` |
| `adversarial-guardrails-schema.json` | `validation/adversarial-guardrails.yaml` | `model::policy::AdversarialGuardrails` |

---

## Entry Point

### `project.yaml`

The single project map. Every LLM agent MUST start by reading this file.

| Section | Purpose |
|--------|-----------|
| `status` | Global project phase: `draft` -> `verified` -> `implementing` -> `implemented` -> `validating` -> `validated`. Per-stage status is stored in `milestones.yaml` |
| `stack` | Technical stack: components, languages, typed dependencies |
| `paths` | Paths to all project directories and files; `paths.llm.map` -> `llm/map.yaml` |
| `constraints` | References to global constraints |
| `validation` | Verification state: `verify_status`, `verify_date`, `issues` |

Format: YAML. Schema: `schema/project-schema.json`. Updated automatically after `/generate` and `/verify`.

---

### `milestones.yaml`

Tracker for the current milestone and history of completed milestones.

| Section | Purpose |
|--------|-----------|
| `project` | Project name |
| `current` | Current active milestone: id, number, branch, stage, `stages[]`, labels, meta |
| `current.stages[]` | List of stages with id, scope, status, commit, `tasks[]`, labels, meta |
| `current.stages[].tasks[]` | Task-level tracking: id, status, `started_at`, `completed_at`, `block_reason`, labels, meta |
| `history[]` | Completed milestones: id, number, status, `contracts[]`, branch, `merged_at` |

Stage statuses: `pending` -> `verified` -> `implementing` -> `implemented` -> `validating` -> `validated`

Stage reopen transitions (via `hlv stage reopen <N>`):
- `implemented` -> `implementing` (manual review found issues)
- `validated` -> `validating` (post-merge issue discovered)
- `validating` -> `implementing` (need more implementation work)

Task statuses: `pending` -> `in_progress` -> `done`, or `blocked` (manual block with reason)

New tasks can be added mid-flight via `hlv task add <ID> <name> --stage <N>`. If the stage is `implemented`, `validated`, or `validating`, it auto-reopens to `implementing`.

`labels` and `meta` on milestone, stage, and task are arbitrary tags and key-value data for clients (Kanban, dashboards). HLV does not use them in core logic.

Format: YAML. Schema: `schema/milestones-schema.json`. Rust model: `model::milestone::MilestoneMap`.

---

### `human/milestones/{id}/plan.md` - milestone table of contents

Lightweight file that always fits into context. Contains no tasks, only the overview.

| Section | Purpose |
|--------|-----------|
| Scope | What this milestone delivers |
| Stages table | Number, scope, task count, budget, status |
| Cross-stage dependencies | Which stages depend on results from earlier ones |

Format: Markdown. Rust model: not parsed as a struct (overview only).

---

### `human/milestones/{id}/stage_N.md` - stage tasks

Self-contained file. `/implement` reads only this file + the contracts.

| Section | Purpose |
|--------|-----------|
| Contracts | List of contracts for this stage |
| Tasks | TASK-NNN with contracts, `depends_on`, output |
| Remediation | FIX tasks from `/validate` (filled when failures occur) |

Tasks without dependencies run in parallel (topological sort).

Format: Markdown. Rust model: `model::stage::StagePlan`.

---

## Human Layer (`human/`)

Intent layer. Owner: human (artifacts) + LLM (contract, glossary, plan generation). The human confirms everything before implementation.

### `human/milestones/{id}/artifacts/` - source artifacts for the milestone

Free-form. The human writes whatever is convenient. The LLM extracts requirements from these artifacts when generating contracts.

Flat directory - one file per feature/topic. Subdirectories are allowed but not required.

Example files:

| File | What it contains |
|------|-------------|
| `checkout.md` | Checkout flow description, cancellation requirements, currencies, UX |
| `why-optimistic-locking.md` | ADR: rationale for choosing optimistic locking |
| `db-constraints.md` | PostgreSQL 16, query time limit, table limit per transaction |

Format: Markdown, arbitrary structure. No format requirements.

---

### `human/glossary.yaml` - domain glossary

Single dictionary of domain types and terms. All contracts MUST reuse glossary types instead of defining their own.

| Section | Purpose |
|--------|-----------|
| `types` | Canonical domain types: `UserId`, `OrderId`, `Money`, `OrderItem` - with kind, format, fields, constraints |
| `enums` | Enumerations: `OrderStatus: [created, paid, cancelled, failed]` |
| `terms` | Canonical terms with definitions and forbidden synonyms |
| `rules` | Usage rules: forbid synonyms for critical entities, require type reuse |

Format: YAML. Schema: `schema/glossary-schema.json`. Generated by the LLM from artifacts, confirmed by the human.

---

### `human/milestones/{id}/contracts/*.md` - contracts (Markdown)

Primary contract format. Full human-readable operation description with links to source artifacts.

Required sections:

| Section | What it contains |
|--------|-------------|
| **Sources** | Links to artifacts the contract was extracted from |
| **Intent** | What the operation does, for whom, call context, quotes from artifacts |
| **Input** | JSON Schema in a YAML block, with `$ref` to glossary types |
| **Output** | JSON Schema in a YAML block |
| **Errors** | Table: error code, HTTP status, condition, source reference |
| **Invariants** | Business invariants: atomicity, non-negativity, debit correctness - with quotes from artifacts |
| **Examples** | At least 1 happy path + 1 error case in JSON (request + response) |
| **Edge Cases** | Specific situations: concurrent access, duplicates, limits - with decision references |
| **NFR** | Latency p99, availability SLO, throughput RPS - YAML block |
| **Security** | Security rules: authn, authz, prepared statements, PII masking |

Every statement MUST reference a source artifact.

Files: `order.create.md`, `order.cancel.md`

---

### `human/milestones/{id}/contracts/*.yaml` - contracts (YAML, machine-readable format)

Structured version of the contract for machine processing. Contains the same data as `.md`, but in a strict format.

| Field | Purpose |
|------|-----------|
| `id`, `version`, `owner` | Contract identification |
| `intent` | Short operation description |
| `inputs_schema` | Input JSON Schema with `$ref` to `glossary.yaml` |
| `outputs_schema` | Output JSON Schema |
| `errors[]` | List of errors: `code`, `when` (formal condition), `http_status` |
| `invariants[]` | Invariants: `id` + `expr` (formal expression) |
| `nfr` | Non-functional requirements: `latency_p99_ms`, `availability_slo`, `throughput_rps_min` |
| `security[]` | Security rules: `rule` ID |
| `compatibility` | Semver compatibility and whether migration is required |
| `depends_on_constraints` | References to global constraint files |

Schema: `schema/contract-schema.json`. `hlv check` uses YAML contracts to extract error IDs, invariant IDs, and constraint rules when validating `@hlv` markers in code.

Files: `order.create.yaml`, `order.cancel.yaml`

---

### `human/constraints/*.yaml` - global constraints

Reusable rules applied to all contracts. Each contract references them via `depends_on_constraints`.

#### `security.yaml`

Global security rules. Owner: `platform-security`.

| Field | Purpose |
|------|-----------|
| `rules[]` | Rule list: `id`, `severity` (critical/high), `statement`, `enforcement` (verification methods: `sast`, `integration_test`, `log_policy_check`, `runtime_scan`) |
| `exceptions` | Exception process: security-team approval required, duration up to 30 days |

Rules: `prepared_statements_only`, `no_secrets_in_logs`, `pii_masking_enabled`, `authn_required`, `authz_order_scope_check`, `request_rate_limit_applied`. Schema: `schema/security-constraints-schema.json`.

#### `performance.yaml`

Global performance limits. Owner: `platform-runtime`.

| Field | Purpose |
|------|-----------|
| `defaults` | Baseline limits: p95/p99 latency, error rate, availability SLO, CPU/memory |
| `overrides[]` | Overrides for specific contracts (for example `order.create` - p99 200ms, 300 RPS) |
| `validation` | Load-testing parameters: warmup, window duration, percentile calculation method |

Schema: `schema/performance-constraints-schema.json`.

---

### `human/milestones/{id}/traceability.yaml` - milestone traceability map

Full chain from requirements to gates: Requirement -> Contract -> Test -> Gate. Per milestone.

| Section | Purpose |
|--------|-----------|
| `requirements[]` | List of requirements with ID and wording, extracted from artifacts |
| `mappings[]` | Mapping from each requirement to contracts, scenarios, tests, and runtime gates |
| `coverage_policy` | Coverage policy: full traceability is mandatory, no unmapped requirements allowed |

Schema: `schema/traceability-schema.json`. Machine-verifiable through `traceability-policy.yaml`.

---

## Validation Layer (`validation/`)

Proof layer. Generated from contracts by `/generate`. Not written by hand, except for static policy files.

### `validation/test-specs/*.md` - test specifications

For each contract, a complete set of tests mapped to gates.

| Test category | What it checks | Derived from |
|-----------------|--------------|----------------|
| **Contract Tests** (CT-*) | Every happy path and error case from the contract | Examples, Errors sections |
| **Property-Based Tests** (PBT-*) | Every invariant across >=10,000 generations | Invariants section |
| **Edge Case Tests** (EC-*) | Boundary situations: concurrent access, duplicates | Edge Cases section |
| **Performance Tests** (PERF-*) | p99 latency, query time under load | NFR section |
| **Security Tests** (SEC-*) | SQL injection, auth, PII masking | Security section |
| **Gate Mappings** | Mapping every test to a gate | - |

Each test includes: Input, Expected, Assertions, Gate.

Files: `order.create.md`, `order.cancel.md`

---

### `validation/scenarios/*.md` - integration scenarios

Expanded scenarios with enough detail to implement integration tests.

| Section | Purpose |
|--------|-----------|
| Intent | What the scenario verifies |
| Preconditions | Initial system state |
| Steps | Table: #, Actor, Action, Expected - step-by-step description |
| Postconditions | State checks after execution |
| Acceptance Criteria | Formal acceptance criteria with IDs |

Files: `checkout.happy-path.md`, `checkout.partial-failure.md`

---

### `validation/traceability.md` - traceability report

Human-readable version of the traceability map with mapping tables and a coverage summary.

| Section | Purpose |
|--------|-----------|
| Requirements | Table of requirements with ID, wording, and source artifact link |
| Mappings | Mapping: Requirement -> Contracts -> Tests -> Gates |
| Coverage Summary | Coverage percentage: requirements, contracts, tests, gates |
| Validation Rules | Checklist of checks from `traceability-policy.yaml` |

---

### Static policy files

Configured once during project initialization (`/init`). Define validation rules.

#### `validation/gates-policy.yaml`

Release gates and thresholds. The gate set depends on the project profile (`hlv init --profile`):

| Profile | Gates |
|---------|-------|
| `minimal` | `contract_tests`, `security` |
| `standard` | + `integration_tests`, `property_based_tests` |
| `full` | + `performance`, `mutation_testing`, `observability` |

`/generate` may adapt the profile based on artifact analysis. Gates can be changed via `hlv gates` or the dashboard.

Each gate has these fields:
- `id` - unique ID (`GATE-*-NNN`)
- `type` - gate type (`contract_tests`, `security`, etc.)
- `mandatory` - whether it blocks the release
- `enabled` - on/off (default: `true`)
- `command` - shell command to run (filled by `/implement`)
- `cwd` - working directory relative to the project root (for example `llm`)
- `pass_criteria` - pass thresholds

Schema: `schema/gates-policy-schema.json`.

#### `validation/equivalence-policy.yaml`

Rules for validating behavioral equivalence when regenerating code.

| Rule | Meaning |
|---------|------|
| `fixed_test_ir` | Compare against a fixed Test IR version and unchanged seed set |
| `nondeterminism_normalization` | Normalize timestamps, UUIDs, `trace_id` |
| `deterministic_external_io` | Record/replay or deterministic mocks |
| `comparison_dimensions` | Response codes, state invariants, side effects |
| `explicit_tolerances` | Numeric values with tolerance, strings with strict equality |

Schema: `schema/equivalence-policy-schema.json`.

#### `validation/traceability-policy.yaml`

Machine-verifiable traceability rules.

| Rule | Meaning |
|---------|------|
| `TRACE-001` | No dangling references |
| `TRACE-002` | Every requirement is reachable through a contract and a test |
| `TRACE-003` | Every test is mapped to a gate |

Also defines ID formats: `REQ-*`, `CTR-*`, `TST-*`, `GATE-*`. Schema: `schema/traceability-policy-schema.json`.

#### `validation/ir-policy.yaml`

Versioning for Contract IR and Test IR.

| Rule | Meaning |
|---------|------|
| `IR-001` | Every IR document includes `ir_schema_version` |
| `IR-002` | Major changes are breaking and require migration |
| `IR-003` | Minor changes may add only backward-compatible fields |
| `IR-004` | Patch changes must not change semantics |
| `IR-005` | Required `source_hash` of normalized artifacts |

Schema: `schema/ir-policy-schema.json`.

#### `validation/adversarial-guardrails.yaml`

Safety rules for adversarial LLMs.

| Rule | Meaning |
|---------|------|
| `ADV-001` | Redact secrets and PII before sending data to the model |
| `ADV-002` | Adversarial model runs in read-only mode without access to prod secrets |
| `ADV-003` | Every finding includes provenance: model, `prompt_hash`, `artifact_hash` |
| `ADV-004` | A finding is accepted only with a reproducible test or enforceable policy rule |

Schema: `schema/adversarial-guardrails-schema.json`.

---

## LLM Layer (`llm/`)

### `llm/map.yaml` - the main project navigator

Authoritative index of all project files and directories. **The single source of truth for file purpose.** File names are arbitrary (`01.rs`, `handler.rs`, `f3a.rs` are all acceptable), so the LLM finds code strictly by descriptions in `map.yaml`, not by file names. Descriptions MUST be sufficient to choose a file without opening it.

`hlv check` validates that every entry from the map exists on disk. If the LLM creates a file but does not add it to the map, `hlv check` will not catch it because the file is outside the index. If the LLM adds an entry but does not create the file, `hlv check` emits `MAP-010`.

| Field | Purpose |
|------|-----------|
| `schema_version` | Map format version |
| `ignore` | Glob patterns excluded from reverse check (MAP-020). Example: `__pycache__`, `*.pyc`, `node_modules`, `target`, `.venv` |
| `entries[]` | List of all project files and directories |
| `entries[].path` | Relative path from the project root |
| `entries[].kind` | `file` or `dir` |
| `entries[].layer` | Layer: `root`, `human`, `validation`, `llm` |
| `entries[].description` | Short description of what the file/directory is for |

Lifecycle:
- `/init` creates the map skeleton (base directories and policy files) and default ignore patterns
- `/generate` adds generated contracts, test specs, scenarios
- `/implement` - agents add every created code/test file; if the stack generates new artifacts, they add ignore patterns
- An empty map (`entries: []`) is allowed - `hlv check` emits info, not an error

Format: YAML. Schema: `schema/llm-map-schema.json`. Path is defined in `project.yaml -> paths.llm.map`.

`hlv check` runs two checks:

1. **Forward** (MAP-010): every map entry exists on disk
2. **Reverse** (MAP-020): every file/directory on disk inside tracked directories is present in the map

Reverse check scans all directories declared as `kind: dir` in the map recursively. Hidden files (`.gitkeep`, `.DS_Store`) are ignored. Files and directories matching `ignore` patterns are ignored (matched both by full path and path components). The `map.yaml` file does not check itself.

Diagnostic codes:

| Code | Severity | When |
|-----|---------|-------|
| `MAP-001` | error | Map file not found |
| `MAP-002` | error | Map parse error |
| `MAP-003` | info | Map is empty (no entries) |
| `MAP-010` | error | A map entry does not exist on disk |
| `MAP-020` | warning | A file on disk is not listed in the map |
| `MAP-100` | info | Forward: N/M entries exist on disk |
| `MAP-101` | info | Reverse: summary result (all ok or N files missing from the map) |

---

## Relationship Between Files

```
human/milestones/{id}/artifacts/**      (written by human)
       │
       ▼
human/glossary.yaml                     (LLM generates shared types and terms)
human/milestones/{id}/contracts/*.md    (LLM generates contracts)
human/milestones/{id}/contracts/*.yaml  (machine-readable version)
human/milestones/{id}/test-specs/*.md   (tests derived from contracts)
human/milestones/{id}/plan.md           (milestone table of contents)
human/milestones/{id}/stage_N.md        (stage tasks)
human/milestones/{id}/traceability.yaml (REQ -> CTR -> TST -> GATE)
human/constraints/*.yaml                (global constraints)
       │
       ▼
llm/map.yaml                            (LLM updates - full file index)
llm/src/                                (LLM agents generate code + inline tests)
       │
       ▼
validation/gates-policy.yaml ──► gates (profile-dependent) ──► Release decision
validation/scenarios/        ──► cross-milestone integration
```

---

### CLI: Gates Management

CRUD management for gates in `validation/gates-policy.yaml`. All changes are saved to the file automatically.

| Command | Description |
|---------|----------|
| `hlv gates` | Show all gates (table: id, type, mandatory, enabled, command) |
| `hlv gates --json` | Output in JSON format |
| `hlv gates add <id> --type <type> [--mandatory] [--command <cmd>] [--cwd <dir>] [--no-enable]` | Add a new gate. `--no-enable` creates it disabled |
| `hlv gates remove <id> [--force]` | Remove a gate. Without `--force`, asks for confirmation |
| `hlv gates edit <id> [--type <type>] [--mandatory \| --no-mandatory]` | Change gate type or mandatory flag |
| `hlv gates run [<id>]` | Run all gates with commands or one specific gate |
| `hlv gates enable <id>` / `hlv gates disable <id>` | Enable / disable a gate |
| `hlv gates set-cmd <id> <cmd>` / `hlv gates clear-cmd <id>` | Set or remove the shell command |
| `hlv gates set-cwd <id> <dir>` / `hlv gates clear-cwd <id>` | Set or remove the working directory |

Gate ID must be unique. Adding a duplicate is an error. When `run` is used, `command` is executed in `cwd` (or the project root), and the result includes exit code + stdout/stderr.

---

### CLI: Constraints Management

CRUD management for constraint files in `human/constraints/`. Each file is a rule-based `ConstraintFile`.

| Command | Description |
|---------|----------|
| `hlv constraints` or `hlv constraints list` | List all constraint files with rule counts |
| `hlv constraints list --severity critical` | Filter by severity (`critical` / `high` / `medium` / `low`) |
| `hlv constraints list --json` | Output in JSON format |
| `hlv constraints show <name> [--json]` | Show the content of a constraint file (all rules, owner, intent) |
| `hlv constraints add <name> [--owner <owner>] [--intent <text>] [--applies-to <scope>]` | Create a new constraint file |
| `hlv constraints remove <name> [--force]` | Remove a constraint file. Without `--force`, confirmation is required |
| `hlv constraints add-rule <constraint> <rule-id> --severity <sev> --statement <text>` | Add a rule to an existing constraint file |
| `hlv constraints remove-rule <constraint> <rule-id>` | Remove a rule from a constraint file |

`<name>` is the file name without extension (for example `security` -> `human/constraints/security.yaml`). Severity values are `critical`, `high`, `medium`, `low`.

---

### CLI: Git Policy

Commit message generation and management of the project's git policy.

| Command | Description |
|---------|----------|
| `hlv commit-msg [--stage] [--type <type>]` | Generate a commit message according to the project convention |

**Git policy in `project.yaml`:**

```yaml
git_policy:
  branch_per_milestone: true          # create a branch during hlv milestone new
  commit_convention: conventional     # conventional / simple / custom
  merge_strategy: squash              # squash / merge / rebase
  branch_prefix: "feature/"           # branch prefix
  custom_template: null               # template for custom convention
```

**`MilestoneGitConfig` in `milestones.yaml`** (optional milestone-level override):

```yaml
current:
  id: "003-payments"
  git:
    branch: "feature/payments"        # overridden branch name
    commit_convention: simple         # overridden convention
```

`hlv commit-msg` reads `git_policy` (or the milestone override), detects the current stage and milestone, and generates the message according to the convention. `--stage` includes the stage number in the message. `--type` sets the commit type (`feat`, `fix`, `refactor`, etc.) for the conventional format.

---

### ConstraintFile Format

Universal constraint file format (`human/constraints/*.yaml`). Used for security, compliance, observability, and any other rule category.

```yaml
id: security                        # unique identifier
version: "1.0.0"                    # semver
owner: platform-security            # owning team
intent: "Global security rules for all contracts"
applies_to: all                     # scope (all / list of contracts)
rules:
  - id: prepared_statements_only
    severity: critical              # critical | high | medium | low
    statement: "All SQL queries must use prepared statements"
    enforcement:
      - sast
      - integration_test
  - id: no_secrets_in_logs
    severity: high
    statement: "Secrets must not appear in logs"
    enforcement:
      - log_policy_check
exceptions:
  process: "Requires security team approval"
  max_duration_days: 30
```

| Field | Type | Description |
|------|-----|----------|
| `id` | string | Unique constraint-file identifier |
| `version` | string | Version in semver format |
| `owner` | string | Owning team |
| `intent` | string | Purpose of the rule set |
| `rules[]` | array | List of rules (`ConstraintRule`) |
| `rules[].id` | string | Unique rule ID (used in `@hlv` markers) |
| `rules[].severity` | enum | Severity: `critical`, `high`, `medium`, `low` |
| `rules[].statement` | string | Rule wording |
| `rules[].enforcement[]` | array | Verification methods (`sast`, `integration_test`, `runtime_scan`, etc.) |
| `exceptions` | object | Exception process (`process`, `max_duration_days`) |

Rust model: `model::policy::ConstraintFile`. Schema: `schema/constraint-schema.json`.

---

### `hlv check` - Constraint Checks

Integrity checks for constraint files, executed by `hlv check`.

| Code | Severity | What it checks |
|-----|---------|--------------|
| `CST-010` | error | Constraint file referenced in `project.yaml -> constraints` is not found on disk |
| `CST-020` | error | Duplicate `rule.id` values within the same constraint file |
| `CST-030` | warning | Invalid `severity` value (allowed: `critical`, `high`, `medium`, `low`) |

These checks run automatically as part of `hlv check`. `CST-010` and `CST-020` block `/verify`.
