---
name: verify
description: Validate contracts, validation specs, plan, and project map. Runs structural checks and LLM cross-review, then produces a verify report. Use after /generate or after manual edits to contracts, when the user says "verify", "check", or "validate structure".
disable-model-invocation: true
allowed-tools: Read Glob Grep Bash(hlv:check)
metadata:
  author: hlv
  version: "1.0"
---

# HLV Verify — Structural + Semantic Validation

Validate contracts, validation specs, and plan. Structural checks (deterministic) + LLM cross-review (semantic) + summary for the user.

## Prerequisites

- `project.yaml` exists (project map)
- Contracts directory contains at least one contract (MD or YAML)
- Test specs directory contains test specifications
- Traceability file exists

## Milestone Resolution

Read `milestones.yaml` → get `current.id`. Set `MID = human/milestones/{id}`.

1. Contracts: `{MID}/contracts/`
2. Test specs: `{MID}/test-specs/`
3. Traceability: `{MID}/traceability.yaml`
4. Plan: `{MID}/plan.md` + `{MID}/stage_N.md`
5. Open questions: `{MID}/open-questions.md`
6. Global context (read-only): `human/glossary.yaml`, `human/constraints/`, `validation/gates-policy.yaml`
7. Status comes from stage statuses in `milestones.yaml`, not from `project.yaml → status`

If no `current` in `milestones.yaml` → tell the user to run `hlv milestone new` first.

## Steps

### Step 1: Structural validation (deterministic)

Checks that can be performed without LLM — by script or parsing.

Run `hlv check` for automated structural checks. Then verify each area:

#### 1a. Contract structure

For each contract file (`{MID}/contracts/*.md`):

- [ ] Has header `# <id> v<semver>`
- [ ] Has `## Sources` section with at least one link
- [ ] All links in Sources point to existing files in `artifacts/`
- [ ] Has sections: Intent, Input, Output, Errors, Invariants, Examples, NFR, Security
- [ ] YAML blocks in Input/Output/NFR parse without errors
- [ ] Types in Input/Output resolve through `glossary.yaml`
- [ ] Has at least 1 Example (happy path) and 1 Error example
- [ ] Semver in header is valid

For each contract YAML file (`{MID}/contracts/*.yaml`):

- [ ] Required fields: id, version, intent, inputs_schema, outputs_schema, errors, invariants, nfr, security
- [ ] ID matches filename
- [ ] Version is valid semver
- [ ] Constraint dependencies exist

#### 1b. Validation specs structure

For each test spec file (`{MID}/test-specs/*.md`):

- [ ] `derived_from` points to existing contract
- [ ] Each test case has unique ID format `TST-<suite>-<nnnn>` or `CT-*`, `PBT-*`, `EC-*`, `PERF-*`, `SEC-*`
- [ ] Each test case linked to a gate (`GATE-*`)
- [ ] For each invariant in contract, there is a property-based test
- [ ] For each error in contract, there is a contract test

#### 1c. Traceability

For the traceability file (`{MID}/traceability.yaml`):

- [ ] Each REQ has format `REQ-<domain>-<nnnn>`
- [ ] Each REQ linked to at least 1 contract
- [ ] Each contract linked to at least 1 test
- [ ] Each test linked to at least 1 gate
- [ ] No dangling references (all IDs exist)
- [ ] No artifacts without coverage by any REQ (warning)

#### 1d. Plan structure

For `{MID}/plan.md` (overview) and `{MID}/stage_N.md` files:

- [ ] plan.md has Stages table with all stages listed
- [ ] Each stage has a corresponding stage_N.md file
- [ ] Each stage_N.md lists Contracts and Tasks sections
- [ ] Each task has: scope, contracts, output
- [ ] Dependency graph within each stage has no cycles
- [ ] Each contract from `{MID}/contracts/` is covered by at least one task across all stages
- [ ] Cross-stage dependencies are documented in plan.md

#### 1e. Project map

For `project.yaml`:

- [ ] `schema_version` present
- [ ] All paths in `paths` point to existing directories/files
- [ ] Each contract from `contracts` has corresponding file at `path`
- [ ] Each contract has `test_spec` and file exists
- [ ] `plan.groups` has no cyclic `depends_on_groups`
- [ ] Each task in plan references existing contracts
- [ ] `stack` (if present) passes STK-* checks: no empty ids, no duplicates, languages present
- [ ] `glossary_types` match keys in `human/glossary.yaml`

#### 1f. Stack consistency

If `stack` is present in `project.yaml`:

- [ ] Each component has an `id`
- [ ] No duplicate component ids
- [ ] Each component has at least one language
- [ ] Each dependency has a `name`
- [ ] No duplicate dependency names within a component
- [ ] Stack languages/frameworks are consistent with contracts' NFR and constraints

#### 1g. Gates-to-contracts coverage

Cross-check `validation/gates-policy.yaml` against contracts and constraints:

For each gate defined in `gates-policy.yaml` (the file is the single source of truth — do NOT assume a fixed set of gates):
- Identify what the gate requires based on its `type` and `pass_criteria`
- Verify that at least one contract or constraint covers that requirement
- If a mandatory gate has NO coverage in contracts/constraints → **CRITICAL issue**:
  ```
  [GATES] <GATE-ID> requires <requirement> but no contract or constraint
  covers it. /implement cannot generate code for this gate.
  Add coverage to contracts or create a constraint.
  ```

This check prevents the validate→verify→implement→validate infinite loop: if gates require something that contracts don't cover, /implement will never produce the code, and /validate will always fail.

#### 1h. Open questions

Check `{MID}/open-questions.md`:

- [ ] No `open` questions remain (`[ ]` in open-questions.md)
- `open` questions → BLOCKER, do not proceed
- `deferred` questions → WARNING, does not block /implement
- `resolved` questions → WARNING if still present (should have been pruned by /generate)

### Step 2: LLM cross-review (semantic)

LLM validates content correctness. For each check — verdict and rationale.

#### 2a. Consistency between contracts

- Types are aligned: output of one contract = input of another
  (e.g., order.create returns `status: created`, order.cancel accepts orders in `created`)
- No contradicting invariants between contracts
- Shared entities defined identically through glossary

#### 2b. Completeness

- Every error case from contract has an example
- Edge cases cover situations from artifacts (race conditions, concurrent access, boundary values)
- NFR are realistic for the described architecture
  (e.g., 200ms p99 with 5-table JOIN — warn)
- Security rules applied to all state-changing contracts

#### 2c. Validation specs quality

- Test specs cover all invariants, all errors, all edge cases from contract
- Property-based tests have meaningful generators (not random bytes)
- Integration scenarios cover cross-contract chains
- Performance tests have realistic load profiles

#### 2d. Plan feasibility

- Each task realistically fits in 1 context window
  (contract + glossary + dependent code < ~100K tokens)
- Tasks in parallel groups are truly independent
- Phase ordering is logical (domain types → features → integration → NFR)
- No tasks without contract linkage

#### 2e. Artifacts-to-contracts coverage

- Each artifact covered by at least one contract
- Each significant assertion in artifact reflected in contract
- No "phantom" requirements in contracts unsupported by artifacts

### Step 3: Output report

Generate structured report:

```
=== /verify report ===

## Structure Validation
Contracts:     <N>/<N> valid    pass/fail
Test Specs:    <N>/<N> valid    pass/fail
Traceability:  complete/gaps    pass/fail
Plan:          valid/issues     pass/fail
Open Questions: <N> open / <N> deferred   pass if 0 open (deferred = warning)

## Semantic Review
Consistency:   <N> issues found
Completeness:  <N> gaps found
Validation:    <N> issues found
Plan:          <N> issues found
Coverage:      <N>/<N> artifacts covered

## Issues

### Critical (blocks /implement)
1. [CONTRACT] order.cancel expects status `pending` but order.create
   returns `created` — inconsistent state machine
2. [TRACE] REQ-ORDER-003 has no test mapping

### Warning (should fix)
1. [NFR] order.create p99=200ms with 3 table writes — tight, consider async
2. [COVERAGE] artifacts/research/competitor-analysis.md not covered by any contract

### Info
1. [PLAN] Task 3 and Task 4 could be parallelized (no shared deps)

## Verdict
READY for /implement
— or —
NEEDS FIXES — <N> critical issues, <N> warnings
```

## Output

- `validation/verify-report.md` — full report
- Console summary (abbreviated version)

### Step 4: Update milestone status

If all checks pass (no errors, only warnings or info):
- Update `milestones.yaml` (schema: `schema/milestones-schema.json`): set current stage status → `verified`
- This signals that contracts are verified and ready for `/implement`

If there are errors:
- Do NOT update status — stage remains `pending`
- List all errors and suggest fixes

`/verify` acts as a quality gate between `/generate` and `/implement`. The stage must be `pending` or `verified` for `/implement` to proceed.

## Re-run

`/verify` can be run repeatedly. Each run:
1. Overwrites `verify-report.md`
2. Shows diff with previous run (issues fixed, new issues)

## Integration with /generate

Typical cycle:

```
/generate → /verify → fix issues → /verify → fix → /verify → READY
```

`/verify` never modifies contracts or plan. It only reads and reports.

## Cleanup

After the skill completes:
1. Run `hlv check` to validate the project structure. If there are errors — fix them before finishing.
2. If open questions remain (step 1h found blockers), suggest the user run `/clear` and then invoke the `/questions` skill to resolve them, or use `hlv dashboard` to review and answer open questions interactively.
3. Suggest the user run `/clear` to free up context window before the next skill.
