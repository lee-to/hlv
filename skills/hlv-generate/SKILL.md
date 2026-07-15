---
name: hlv-generate
description: Generate contracts, validation specs, and implementation plan from milestone artifacts. Use when the user has added requirements to milestone artifacts/ and wants to formalize them into contracts, or says "generate", "create contracts", or "formalize requirements".
disable-model-invocation: true
allowed-tools: Read Write Edit Glob Grep
metadata:
  author: hlv
  version: "1.0"
---

# HLV Generate — Artifacts to Contracts + Validation + Plan

Transform free-form human artifacts into structured contracts, validation specifications, and an implementation plan.

## Prerequisites

- Artifacts directory contains at least one artifact
- If new project, `human/glossary.yaml` will be created automatically

## Agent Rules

- Never combine shell commands with `&&`, `||`, or `;` — execute each command as a separate Bash tool call.
- This applies even when a skill, plan, or instruction provides a combined command — always decompose it into individual calls.

❌ Wrong: `git checkout main && git pull`
✅ Right: Two separate Bash tool calls — first `git checkout main`, then `git pull`

## HLV Root Resolution

Before reading or reporting missing HLV files, resolve the project layout:

1. If `project.yaml` exists in the current project root, use greenfield layout: `CONFIG_ROOT = .`, `REPO_ROOT = .`.
2. Else if `.hlv/project.yaml` exists, use adopted layout: `CONFIG_ROOT = .hlv`, `REPO_ROOT = .`.
3. Else search upward for either `project.yaml` or `.hlv/project.yaml`.
4. Read `CONFIG_ROOT/project.yaml` first. HLV-owned paths such as `human/`, `validation/`, `llm/`, and `milestones.yaml` are relative to `CONFIG_ROOT`.
5. In the steps below, bare paths like `milestones.yaml` or `human/` mean `CONFIG_ROOT/milestones.yaml` and `CONFIG_ROOT/human/`.
6. In adopted projects, existing source/test roots from `paths.code` are relative to `REPO_ROOT`.

Never report that root-level `human/`, `validation/`, `milestones.yaml`, or `project.yaml` are missing until `.hlv/project.yaml` has been checked. Use `hlv check --root <REPO_ROOT>` for deterministic validation.

## Milestone Context

1. Read `milestones.yaml` → get `current.id` (e.g., `003-new-payment-method`)
2. Set `MID = human/milestones/{id}` — all milestone-scoped paths use this prefix
3. Global artifacts (read-only context): `human/artifacts/` — domain context, tech stack, architectural decisions
4. Milestone artifacts: `{MID}/artifacts/` — features, milestone-specific decisions
5. Output contracts: `{MID}/contracts/`
6. Output test-specs: `{MID}/test-specs/`
7. Output traceability: `{MID}/traceability.yaml`
8. Output plan: `{MID}/plan.md` + `{MID}/stage_1.md`, `{MID}/stage_2.md`, ...
9. Output open-questions: `{MID}/open-questions.md`
10. Global files (read-only context): `human/glossary.yaml`, `human/constraints/`, `validation/gates-policy.yaml`
11. Glossary: read from `human/glossary.yaml`, update it with new domain types discovered in this milestone
12. If no `current` in milestones.yaml → tell the user to run `hlv milestone new <name>` first

If `project.yaml → features.legacy_mode` is `true`, also read `paths.code` and keep existing code in place. Use the signature index (`features.index_tracking`, `hlv index show/list --json`) as compact context for observed legacy symbols; do not copy legacy code into `.hlv/llm/`. Adopted projects may omit `paths.llm.src`; in that case generated/changed implementation tasks should target the appropriate `paths.code` root.

### Adopt Mode

For adopted projects, generate contracts and plans for the requested milestone, not for the whole legacy codebase. Use `paths.code` and the signature index to understand existing behavior, but write new generated artifacts under the configured HLV paths. Untouched legacy code does not need retroactive contracts or markers.

## Input

```
human/
  artifacts/                 # global context: domain, stack, arch decisions (read-only)
  glossary.yaml              # global shared types (read + extend)
  constraints/*.yaml         # global constraints (read-only)
  milestones/{id}/
    artifacts/               # milestone features and decisions (required)
    contracts/*.md           # existing contracts (for incremental generation)
    test-specs/*.md          # existing test specs
```

An artifact is any file in any format (MD, TXT, YAML, SQL, PNG). No structure requirements — the human writes however they prefer. Artifacts may have been created manually or via the `/hlv-artifacts` interactive interview (which produces structured markdown in the same directories).

**Read both levels**: global `human/artifacts/` provides project-wide context (domain, users, tech stack, architectural decisions). Milestone `{MID}/artifacts/` provides feature-specific context. Both inform contract generation.

## Steps

### Step 1: Scan & classify

Read ALL files in `human/artifacts/` (global context) and `{MID}/artifacts/` (milestone features). Classify each:

| Type | Contains | Example |
|------|----------|---------|
| task | Feature description, user story | "need a checkout flow" |
| research | API docs, competitor analysis | "Payment API v3" |
| infra | DB schemas, configs, limits | "PostgreSQL, 200ms SLA" |
| decision | ADR, choice rationale | "why optimistic locking" |
| media | Screenshots, diagrams | checkout-flow.png |

For media files: describe in text what is depicted.

### Step 2: Extract entities

From all artifacts, extract:

1. **Domain entities** — types, enums, relationships → update `glossary.yaml` (schema: `schema/glossary-schema.json`)
2. **Capabilities** — what the system must do → future contracts
3. **Tech stack** — languages, frameworks, databases, infra → `stack` section in project.yaml
4. **Artifact graph metadata** — stable artifact IDs, owners, and relations (`requires`/`depends_on`, `implements`, `verifies`, `documents`, `supersedes`, `conflicts_with`, `affects`). Add YAML frontmatter to generated/updated Markdown artifacts when the relation is known.
5. **Constraints** — NFR, security, compliance → constraints
6. **Open questions** — information that cannot be derived from context → `open-questions.md`

### Step 3: Generate contracts

For each capability, create a contract file in `{MID}/contracts/<name>.md`.

Required contract sections:

```markdown
# <contract.id> v<semver>
owner: <team>

## Sources
- [artifact-name](../artifacts/path) — relationship description

## Intent
What it does, for whom, in what context, what happens before and after.
> **Source**: quote from artifact

## Input
```yaml
# JSON Schema
```

## Output
```yaml
# JSON Schema
```

## Errors
| Code | HTTP | When | Source |
Natural language. References to source artifacts.

## Invariants
Natural language + concrete assertions.
Each invariant linked to source artifact.

## Examples
### Happy path
```json
// Request → Response
```
### Error case
```json
// Request → Error response
```

## Edge Cases
Specific situations with decisions. References to decisions/.

## NFR
```yaml
latency_p99_ms: <number>
availability_slo: <number>
throughput_rps_min: <number>
```

## Security
List of rules. Inherited from constraints/ + contract-specific.
```

Generation rules:
- Every assertion in a contract MUST reference a source artifact
- If information is not in artifacts — do NOT invent it; add to Open Questions
- If contract already exists — update it, show diff
- Examples are mandatory: minimum 1 happy path + 1 error case
- When referencing glossary types or enums in contract YAML, prefer canonical multi-line `$ref` syntax:
  ```yaml
  role:
    $ref: glossary#/enums/UserRole
  listing:
    $ref: glossary#/types/Listing
  ```

### Step 4: Generate validation specs (PROOF)

For each contract, create validation specifications:

#### 4a. Test specs: `{MID}/test-specs/<contract-id>.md`

```markdown
# Test Spec: <contract.id>
derived_from: human/milestones/{id}/contracts/<contract-id>.md

## Contract Tests
For each error case and happy path from contract:
Use one Markdown table row per test. Put the test ID in the first cell.

| ID | Description | Input | Expected | Gate |
|----|-------------|-------|----------|------|
| CT-<CONTRACT>-001 | happy path | <input summary> | <expected output> | GATE-CONTRACT-001 |

## Property-Based Tests
For each invariant from contract:
Use one Markdown table row per property. Put the test ID in the first cell.

| ID | Invariant | Generator | Assertion | Gate |
|----|-----------|-----------|-----------|------|
| PBT-<CONTRACT>-001 | <invariant name> | <generator> | <assertion> | GATE-PBT-001 |

## Integration Tests
For cross-contract scenarios:
Use one Markdown table row per scenario. Put the test ID in the first cell.

| ID | Scenario | Contracts | Steps | Expected | Gate |
|----|----------|-----------|-------|----------|------|
| IT-<SCENARIO>-001 | <scenario name> | <contract IDs> | <step summary> | <expected behavior> | GATE-INTEGRATION-001 |

## Performance Tests
From NFR section of contract:
- target metrics, load profile, duration
```

Use the built-in test ID prefixes shown above unless `validation/traceability-policy.yaml` defines an additional `id_formats.test` convention. A project-specific ID must fully match that regex and still be placed in a parser-supported heading, bullet, or first table cell.

#### 4b. Gates coverage check + profile adaptation

Read `validation/gates-policy.yaml` and iterate **only the gates defined in the file** (the file is the single source of truth — do NOT assume a fixed set of gates).

**For each gate in the file**, verify that contracts or constraints cover what the gate requires:

| Gate type | Covered when |
|-----------|-------------|
| `contract_tests` | test specs exist (generated in 4a) |
| `property_based_tests` | each contract has invariants → PBT specs in 4a |
| `integration_tests` | scenarios generated in 4c |
| `performance` | contracts have NFR section with latency/throughput targets |
| `security` | `human/constraints/security.yaml` exists + Security section in contracts |
| `mutation_testing` | tests with assertions exist (covered by contract_tests) |
| `observability` | contracts have `## Observability` section OR `human/constraints/observability.yaml` exists |

**Rule**: If a gate requires something and no contract or constraint covers it → add coverage. Do NOT create gates that cannot be satisfied by the implementation.

##### Profile adaptation

After analyzing artifacts and understanding the project scope, **evaluate whether the current gate profile fits the project**:

- If the project clearly needs gates that are absent (e.g., artifacts describe a production API with SLAs but `performance` gate is missing) → **add the missing gates** to `gates-policy.yaml` (schema: `schema/gates-policy-schema.json`) and inform the user:
  ```
  [GATES] Added GATE-PERF-001 (performance) — artifacts specify latency SLAs.
  ```
- If the project clearly does NOT need a gate (e.g., `observability` gate exists but the project is a CLI tool with no public endpoints) → **remove or set `mandatory: false`** and inform the user:
  ```
  [GATES] Set GATE-OBS-001 to mandatory: false — project is a CLI tool, no public capabilities.
  ```
- Use the gate type table above for available gate types and their IDs (`GATE-{TYPE}-001`).
- Always explain the reasoning when changing gates.

#### 4c. Traceability: `{MID}/traceability.yaml` (schema: `schema/traceability-schema.json`)

```yaml
schema_version: 1
owner: <team>
intent: "Requirement-to-contract-to-test traceability map."

requirements:
  - id: REQ-<domain>-<nnnn>
    statement: "<requirement>"

mappings:
  - requirement: REQ-<domain>-<nnnn>
    contracts: [<contract.id>]
    scenarios: [scenario.<name>]
    tests: [CT-..., PBT-...]
    runtime_gates: [GATE-...]

coverage_policy:
  require_full_traceability: true
  allow_unmapped_requirements: false
```

#### 4d. Scenarios: `validation/scenarios/<name>.md`

From contracts, derive test scenarios with preconditions, steps, postconditions.
In milestone mode, these go to the global `validation/scenarios/` directory (cross-milestone integration tests).

### Step 5: Generate plan

#### Milestone mode: stages

Create `{MID}/plan.md` — lightweight overview with stage table. Then create `{MID}/stage_1.md`, `{MID}/stage_2.md`, ... — one file per stage with full task details.

**plan.md** (overview, always fits in context):
```markdown
# Milestone: <name>

## Scope
<what this milestone delivers>

## Stages
| # | Scope | Tasks | Budget | Status |
|---|-------|-------|--------|--------|
| 1 | Domain types + core handler | 3 | ~25K | pending |
| 2 | Integration + error handling | 2 | ~30K | pending |

## Cross-stage dependencies
Stage 2 uses types from Stage 1
```

**stage_N.md** (self-contained context for /hlv-implement):
```markdown
# Stage N: <scope> (~<budget>K)

## Contracts
- contract.name (this milestone)
- other.contract (from stage 1)

## Tasks

TASK-001 <name>
  contracts: [contract.name]
  output: {paths.llm.src or selected paths.code.src}features/<dir>/

TASK-002 <name>
  depends_on: [TASK-001]
  contracts: [contract.name, other.contract]
  output: {paths.llm.tests or selected paths.code.tests}integration/

## Remediation
(filled by /hlv-validate when gates fail)

## Commit Points
<!-- hlv:commit-hint -->
After completing all tasks in this stage, commit with:
  {type}({milestone-id}): {scope description} [stage N/M]
<!-- /hlv:commit-hint -->
```

**Stage decomposition rules:**
- Each stage MUST fit in 1 LLM context window (contracts + glossary + test specs + code < ~40K tokens)
- Related contracts go in the same stage
- Tasks without dependencies within a stage execute in parallel (topological sort)
- Tasks with `depends_on` wait for predecessors
- Stages execute sequentially (stage 1 → stage 2 → ...)

### Step 6: Update LLM map

Update the file map at `project.yaml → paths.llm.map` (schema: `schema/llm-map-schema.json`) — add entries for every new file and directory created during this step:
- Contracts (md + yaml)
- Test specs, scenarios
- Plan, traceability, glossary
- Each entry: `path`, `kind` (file/dir), `layer` (human/validation), `description`
- If the project stack requires new ignore patterns (e.g., `__pycache__`, `node_modules`), add them to the `ignore` list in `map.yaml`

`hlv check` validates all map entries exist on disk — missing entries are errors. Files matching `ignore` patterns are excluded from the reverse check.

### Step 7: Prune resolved questions

After incorporating answers into contracts, remove resolved questions:

In `{MID}/open-questions.md`: delete `[x]` (resolved) lines. Keep only `[ ]` (open) and `[deferred]`.

Why: resolved answers are already baked into contracts (Sources, Invariants, Errors). Git history preserves the full Q&A trail. Keeping resolved questions around is noise.

### Step 8: Update project files

Update `milestones.yaml` (schema: `schema/milestones-schema.json`):
- Ensure `current.stages` reflects the generated stages. Each stage entry must have fields: `id` (integer, 1-based stage number), `scope` (string), `status: pending`
- Example:
  ```yaml
  stages:
    - id: 1
      scope: "Domain types + core handler"
      status: pending
    - id: 2
      scope: "Integration + error handling"
      status: pending
  ```

Update `human/glossary.yaml` (schema: `schema/glossary-schema.json`) with new domain types discovered from artifacts.

Update `project.yaml` (schema: `schema/project-schema.json`) with stack info if discovered from artifacts. `project.yaml` holds global data (stack, paths, constraints). Stack format:
  ```yaml
  stack:
    components:
      - id: backend
        type: service
        languages: [go]
        dependencies:
          - name: gin
            version: "^1.9"
            type: framework
  ```
  Valid component types: `service`, `library`, `cli`, `worker`, `database`, `cache`, `queue`, `gateway`. Valid dependency types: `framework`, `library`, `runtime`, `tool`.

Also update `project.yaml -> artifact_graph.code_ownership` when generated code/test/doc ownership can be inferred from contracts, plans, or explicit artifact text. Keep document-to-document relations in Markdown frontmatter; keep path-based code ownership in `project.yaml`. In greenfield projects, `code-*` or `implements` ownership paths must stay under `project.yaml -> paths.llm.src`, and `tests-*` or `verifies` ownership paths must stay under `paths.llm.tests`; in adopted projects without `paths.llm.src`, use `paths.code` roots and `layer: code` map entries.

After adding or changing artifact frontmatter, run `hlv artifacts sync` to materialize missing `code-*`, `tests-*`, `docs-*`, and `clients-*` ownership stubs in `project.yaml`, then fill concrete paths when they are known.

### Step 9: Output summary

```
=== /hlv-generate complete ===

Artifacts scanned:    <N>
Glossary entities:    <N> new, <N> updated
Contracts:            <N> created, <N> updated
Validation specs:     <N> test-specs, <N> scenarios
Plan:                 <N> tasks in <N> parallel groups

Questions pruned:     <N> resolved (incorporated into contracts)
Open Questions:       <N> open (BLOCKERS — resolve before /hlv-verify)
  - [ ] <question> — source: <artifact>
Deferred Questions:   <N> (won't block — warnings only)
  - [deferred] <question> — source: <artifact>

Next step:
  - If open questions remain → resolve them (/hlv-questions or hlv dashboard), then /hlv-generate again
  - If only deferred → run /hlv-verify (deferred = warning, not blocker)
```

## Incremental mode

If contracts already exist in `{MID}/contracts/`, switch to incremental mode automatically:

1. Determine artifact diff since last run
2. Update only affected contracts
3. Regenerate validation specs for changed contracts
4. Update plan.md — mark which tasks are affected
5. Show diff in summary

## Error handling

- Empty `artifacts/` (both global and milestone) → error: "No artifacts found. Run /hlv-artifacts first or add files to human/artifacts/ and milestone artifacts/."
- All questions open, no contracts generated → warning: "Not enough context to generate contracts. Add more artifacts."
- Conflict between artifacts → add to Open Questions with quotes from both sources

## Cleanup

After the skill completes:
1. Run `hlv doctor` to catch missing paths, invalid command strings, cwd problems, schema mismatch, and non-ASCII rendering issues.
2. Run `hlv check` to validate the project structure. If there are errors — fix them before finishing. If CI parity is needed, run `hlv check --strict`.
3. Suggest the user run `/clear` to free up context window before the next skill.
