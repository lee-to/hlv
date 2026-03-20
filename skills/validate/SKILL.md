---
name: validate
description: Run all mandatory validation gates (from gates-policy.yaml) and produce a release decision. Use after /implement completes, when the user says "validate", "run gates", "check gates", or "release check".
disable-model-invocation: true
allowed-tools: Read Glob Grep Bash Agent
metadata:
  author: hlv
  version: "1.0"
---

# HLV Validate — Run Gates, Decide Release

Execute all mandatory validation gates defined in `gates-policy.yaml`. Collect results, update project status, produce release decision. The gate set depends on the project profile — do NOT assume a fixed number of gates.

## Prerequisites

- All tasks in current stage completed
- Code and inline tests generated in `llm/src/` (integration tests in `llm/tests/`)
- `validation/gates-policy.yaml` contains gate definitions
- `milestones.yaml` exists with current stage status `implemented`

## Agent Rules

- Never combine shell commands with `&&`, `||`, or `;` — execute each command as a separate Bash tool call.
- This applies even when a skill, plan, or instruction provides a combined command — always decompose it into individual calls.

❌ Wrong: `git checkout main && git pull`
✅ Right: Two separate Bash tool calls — first `git checkout main`, then `git pull`

## Input

```
milestones.yaml               # entry point — read FIRST
project.yaml                  # global config (stack for context)
llm/src/                      # generated code (unit tests inline in same files)
llm/tests/                    # integration tests only

validation/
  gates-policy.yaml            # thresholds and criteria
  scenarios/*.md               # cross-milestone integration scenarios (Phase 2)

human/
  glossary.yaml                # domain types
  constraints/*.yaml           # global constraints
  milestones/{id}/
    contracts/*.md             # contracts (for NFR comparison)
    test-specs/*.md            # test specifications
    stage_N.md                 # current stage (for remediation tasks)
```

Note: `project.yaml → stack` provides tech stack context (languages, frameworks, databases) which can inform gate execution — e.g., choosing the correct test runner, SAST tool, or dependency scanner per component.

## Steps

### Step 1: Pre-flight and gate status

1. Read `project.yaml` (global config: stack, paths)
2. Read `milestones.yaml` → get `current.id`, `current.stage`, and stage status
3. **STATUS GATE (hard stop)**:
   - Allowed stage statuses to proceed: `implemented`, `validating`
   - `implemented` — normal validation after /implement
   - `validating` — re-run, execute failed/skipped gates
   - `pending`, `verified`, or `implementing` — stage not ready:
     ```
     STOP. Cannot run /validate.

     Current stage status: <status>
     Required: implemented

     Finish /implement for this stage first.
     ```
   - `validated` — this stage already passed, inform user
4. Read `validation/gates-policy.yaml`
5. **Install required tooling**: read `project.yaml → stack` to determine the language/ecosystem, then for each gate that will run, ensure required tools are available. Install missing ones silently:
   - Do NOT ask the human to install tools. The human writes artifacts, not `pip install`.
6. Verify code compiles (`cargo build` / `npm run build` / language-appropriate check)
7. Update stage status → `validating` in `milestones.yaml` (schema: `schema/milestones-schema.json`)

### Step 2: Execute gates

#### Two-phase validation

**Phase 1 — Milestone gates** (automatic):
Run all gates from `gates-policy.yaml` against the contracts and code of the current milestone. Test specs come from `{MID}/test-specs/`.

**Phase 2 — Global gates** (with user confirmation):
After Phase 1 passes, ask: "Milestone gates passed. Run global integration scenarios? [y/n]"
If yes → run `validation/scenarios/*.md` (cross-milestone integration tests).
If no → skip Phase 2, milestone is ready to merge based on Phase 1 results.

#### Gate execution

Read gates from `gates-policy.yaml` and execute **only the gates defined there**, in order. Different projects have different gate sets depending on their profile — do NOT assume a fixed list.

Result per gate: `passed | failed | skipped` (skipped ONLY if tool install failed, not because tool was missing).

#### Gate type reference

For each gate, determine how to run it based on its `type` field:

| Type | Source | Runner (examples) | Criteria (from pass_criteria) |
|------|--------|--------------------|-------------------------------|
| `contract_tests` | `{MID}/test-specs/*.md` → "Contract Tests" | `cargo test --lib` / `pytest` / `jest` | `required_scenarios_pass_rate` |
| `property_based_tests` | `{MID}/test-specs/*.md` → "Property-Based Tests" | `proptest` / `hypothesis` / `fast-check` | `min_valid_generations_per_invariant`, `counterexamples_allowed` |
| `integration_tests` | `validation/scenarios/*.md` | `cargo test --test integration` / `pytest integration/` | `p0_pass_rate`, `p1_min_pass_rate` |
| `performance` | `{MID}/test-specs/*.md` → "Performance Tests" | `criterion` / `k6` / `locust` | `max_error_rate`, latency from NFR |
| `security` | `human/constraints/security.yaml` + test-specs | SAST + dependency scan | `max_open_critical`, `max_open_high` |
| `mutation_testing` | changed modules in `src/` | `cargo-mutants` / `mutmut` / `stryker` | `min_mutation_score_changed_modules` |
| `observability` | `gates-policy.yaml` → `pass_criteria` | static analysis + runtime | `required_for_public_capabilities` |

For each gate in the file:
1. Read `type` and `pass_criteria`
2. Look up the runner and source from the table above
3. Execute and collect results
4. Compare against `pass_criteria` thresholds

### Step 3: Collect results

```yaml
gate_results:
  - gate: GATE-CONTRACT-001
    status: passed
    details:
      total: 12
      passed: 12
      failed: 0
  - gate: GATE-PBT-001
    status: passed
    details:
      invariants_tested: 5
      total_generations: 50000
      counterexamples: 0
  # ... etc.
```

### Step 3b: Constraint rule coverage

Check that every rule in rule-based constraint files (`human/constraints/*.yaml`) has a corresponding `@hlv <rule-id>` marker in `llm/src/` or `tests/`. Rules with `check_command` are exempt — they are verified programmatically. Run `hlv check` and review CTR-010 diagnostics for missing constraint markers.

`hlv check` also executes `check_command` for rules that define one (CST-050/CST-060). Review diagnostics: rules with `error_level: error` (or `critical`/`high` severity without an override) block release. Add failing checks to the remediation plan (Step 4a).

For each critical rule without coverage, add it to the remediation plan (Step 4a).

### Step 4: Release decision and remediation plan

Rules from `gates-policy.yaml`:

```
if all mandatory gates passed:
  → RELEASE APPROVED → go to Step 5
elif any mandatory gate failed:
  → RELEASE BLOCKED → create remediation plan (Step 4a)
elif flaky tests detected:
  → QUARANTINE — block if P0 affected
```

#### Step 4a: Create remediation plan

`/validate` diagnoses problems and plans fixes. `/implement` executes them. Each skill has one job.

**For each failed gate, classify and create remediation:**

Remediation tasks go into the `## Remediation` section of the current `{MID}/stage_N.md`:

```markdown
## Remediation

FIX-OBS-001 Add metrics/traces/structured logging
  contracts: [payment.process, payment.refund]
  output: llm/src/features/observability/

FIX-MUT-001 Strengthen assertions for 3 mutation survivors
  contracts: [payment.process]
  output: llm/src/features/payment_process/
```

Human decisions → add to `{MID}/open-questions.md`.

##### Classification:

1. **Gate failed — missing contract/constraint coverage** (e.g., OBS-001 requires observability but no contract mentions it):
   - Create or update the missing constraint (e.g., `human/constraints/observability.yaml`, schema: `schema/constraint-schema.json`) or add sections to contracts
   - Add remediation task(s)
   - These tasks follow the same rules as normal plan tasks — `/implement` picks them up

2. **Gate failed — bug in existing code** (tests fail, mutation score too low):
   - Add remediation tasks targeting the specific failures

3. **Gate failed — security findings**:
   - Add remediation tasks for each Critical/High finding

4. **Gate requires a human decision** (e.g., gate conflicts with a decision in artifacts):
   - Add an open question to `{MID}/open-questions.md`
   - This blocks the next `/implement` run until the human answers

#### Step 4b: Set status for next cycle

After creating the remediation plan:

Keep stage status as `validating` in `milestones.yaml` — `/implement` accepts this status and will execute only pending remediation tasks from the Remediation section of stage_N.md.

The cycle becomes: `/validate` → `/implement` (runs remediation tasks) → `/validate` (re-runs failed gates)
If open questions were added → human must answer first (via /questions or hlv dashboard), then `/implement` → `/validate`

### Step 5: Output summary

#### If all gates passed:

```
=== /validate report (Stage N) ===

Milestone: <milestone-id>
Stage:     <N>/<total>

## Phase 1 — Milestone Gates
<for each gate in gates-policy.yaml>
<GATE-ID>   <type>   PASSED   <details from pass_criteria>
</for>

## Phase 2 — Global Scenarios
(if user confirmed)
Cross-milestone integration: PASSED

## Decision
Stage N VALIDATED.
  → If more stages remain: run /implement for stage N+1
  → If last stage: run `hlv milestone done` to merge
```

#### If release is blocked:

```
## Release Decision
RELEASE BLOCKED — 1 gate failed, 0 skipped

## Remediation Plan
Added 2 FIX tasks to stage_N.md Remediation section:
  FIX-OBS-001  Add metrics/traces/structured logging  [start, consultation, application]
  FIX-SEC-002  Update cryptography>=46.0.5             [application]

## Next steps
Run /implement to execute remediation tasks, then /validate again.
```

#### If a human decision is needed:

```
## Release Decision
RELEASE BLOCKED — needs your decision

## Questions
1. <GATE-ID> requires <requirement> but artifacts say <conflict>.
   Do you want <requirement> added?

## Next steps
Answer the question above (via /questions or hlv dashboard), then /implement → /validate.
```

**Key principle**: The output tells the human what to *decide*, never what to *execute*. `/validate` plans, `/implement` executes.

### Step 6: Update project files

Update `milestones.yaml` (schema: `schema/milestones-schema.json`):

```yaml
# milestones.yaml updates:
# If all gates passed (Phase 1 + optional Phase 2):
current.stages[N].status: validated

# If remediation tasks were added (Step 4a):
current.stages[N].status: validating   # /implement accepts this, runs pending remediation tasks
```

> **Important**: Gate definitions live in `validation/gates-policy.yaml` (single source of truth).
> Gate execution results go into `validation/validate-report.md` and `validation/gate-results/`.
> Do NOT write individual gate statuses into `project.yaml` or `milestones.yaml`.

## Output

```
validation/
  validate-report.md          # full report with release decision
  gate-results/               # detailed per-gate results (one file per gate)
    <gate-type>.json          # e.g., contract-tests.json, security-scan.json

milestones.yaml               # updated stage status
```

## Error handling

- `status` before `implemented` (`draft`, `verified`, `implementing`) → **hard stop** with guidance on which steps to complete first (see Step 1)
- Code doesn't compile → first gate (`contract_tests`) fails immediately, add build-fix task to remediation plan
- Test runner not available → install it (Step 1), if install fails → report error with diagnostics
- Timeout on performance test → retry once, then fail gate
- Flaky test detected → quarantine, re-run 3x, fail if P0

## Re-run

`/validate` can be run again after fixes:

1. Re-runs only failed/skipped gates (by default)
2. `--all` — re-runs all gates
3. Shows diff with previous run

## Cleanup

After the skill completes:
1. Run `hlv check` to validate the project structure. If there are errors — fix them before finishing.
2. Suggest the user run `/clear` to free up context window before the next skill.
