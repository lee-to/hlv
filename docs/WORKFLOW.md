# HLV Workflow - from zero to the first feature

## Problem

You have an idea (or a task, or an entire service). You know what needs to be built, but it lives in your head, your notes, and your Slack threads. How do you turn that into working code with guarantees?

HLV is the pipeline:

```
your notes -> formal contracts -> validation -> code -> proof
```

## Two Modes of Work

Every change is a separate milestone with its own contracts, stages, and tests. This scales to dozens of iterations.

`hlv init` creates the first milestone automatically.

---

## Workflow

```
hlv init -> hlv milestone new "feature"
    -> /artifacts -> /generate -> /verify
    -> /implement (stage 1) -> /validate
    -> /implement (stage 2) -> /validate
    -> hlv milestone done
    -> hlv milestone new "next-feature" -> ...
```

### 1. Bootstrap + first milestone

```bash
hlv init --project payments --owner backend-team
# init asks for:
#   - gate profile (minimal/standard/full)
#   - first milestone name
#   - feature flags: linear architecture (Y/n), @hlv markers (Y/n)
# --profile minimal|standard|full can be passed explicitly
```

Feature flags (`features.linear_architecture`, `features.hlv_markers`) control whether HLV's opinionated code style and `@hlv` marker system are enforced. Both default to `true`. Set to `false` in `project.yaml` to opt out.

### 2. Fill in the context

```bash
/artifacts     # interview -> human/milestones/001-order-create/artifacts/
```

### 3. Generate contracts

```bash
/generate      # artifacts -> contracts, test-specs, plan.md + stage_N.md
```

### 4. Check

```bash
hlv check      # structural validation
/verify        # semantic validation
```

### 5. Implement by stages

```bash
/implement     # executes the current stage (stage_1.md)
/validate      # checks gates for stage 1
/implement     # next stage (stage_2.md)
/validate      # checks gates for stage 2
```

### 6. Finish the milestone

```bash
hlv milestone done    # merge and move to history
```

### 7. Next change

```bash
hlv milestone new "add-payments"    # new milestone, new cycle
```

---

## Details of Each Phase

### Phase 1: Context dump - write however you want

**Who does it**: you (the human). Only you.

This is the most important step. You dump everything you know about the task into `human/milestones/{id}/artifacts/`. Format - any. Structure - any. Language - any.

### Two ways to populate `artifacts/`

**Option 1: Manually.** Create files in `human/milestones/{id}/artifacts/` however you want - markdown, plain text, screenshots, SQL dumps. No format requirements.

**Option 2: `/artifacts` - interactive interview.** The LLM asks questions, you answer, and it writes the answers into the right files.

```
/artifacts                   # new milestone - full interview
/artifacts                   # existing milestone - only new/changed information
```

`/artifacts` runs 5 blocks:

| Block | What it discovers | Where it writes |
|------|-------------|-----------|
| Domain & Users | What the system does, who the users are, business context | `artifacts/context.md` |
| Features & Flows | Operations, steps, errors, business rules | `artifacts/<feature>.md` (one file per feature) |
| Infrastructure | Stack, DB, external APIs, limits | `artifacts/stack.md`, `artifacts/constraints.md` |
| Decisions | What was decided, why, what alternatives were rejected | `artifacts/<decision>.md` |
| Unknowns & Risks | What we do not know, what worries us, what dependencies exist | `artifacts/unknowns.md` |

After each block the LLM shows the written file and asks: *"Is this correct? What should be added?"*

In incremental mode (when artifacts already exist), the LLM reads the existing artifacts first, shows what is already known, and asks only about new information.

> **When to use what**: `/artifacts` is a good starting point if you do not know where to begin. Manual authoring is better if you already have notes, conversations, or formal specs. You can combine both: start with `/artifacts`, then add files manually.
>
> **Already have a spec?** Drop it into `human/milestones/001-init/artifacts/` (any format — markdown, plain text, PDF export, etc.) and run `/artifacts`. The skill will detect existing files, read them, and generate global artifacts (`human/artifacts/context.md`, `stack.md`, `constraints.md`, etc.) based on the information in your spec. This way you skip the interview and immediately get a structured context layer for `/generate`.
>
> **Language selection policy**: by default HLV recommends strict, compile-time-safe languages where they naturally fit the task. But without dogma: for UI, TypeScript is usually better; for bot/automation/SDK-first tasks, Python, TypeScript, or another language with a better ecosystem fit may be more appropriate. For ML/data and complex AI-chain tasks, Python may also be the best deliberate choice because of the ecosystem. If the choice is not obvious, it should be explicitly recorded in artifacts or raised as an open question instead of guessed.

### What to put into `artifacts/`

Flat directory - one file per topic. Subdirectories are allowed, but not required.

```
human/milestones/{id}/artifacts/
  checkout.md            - what needs to be built (features, user stories)
  db-constraints.md      - DB schemas, configs, limits from DBA/DevOps
  optimistic-locking.md  - why this approach was chosen (ADR)
  context.md             - research, API docs, platform constraints
```

### Examples of good artifacts

**Feature** (`checkout.md`):

```markdown
The user clicks "Place order", and the system creates an order from the cart.
Payment is a separate step after the order is created.
There must be a way to cancel the order while it is unpaid.
Maximum 200ms p99.
If an item is out of stock, show which one and how many remain.
```

**Infrastructure** (`infra/db.md`):

```markdown
PostgreSQL 16, pool of 20 connections.
DBA said: no more than 3 tables in one transaction, max query 100ms.
Tables: users, orders, order_items, inventory, payments.
```

**Decision** (`decisions/optimistic-locking.md`):

```markdown
The problem: two users may try to buy the last item at the same time.
Decision: optimistic locking through a version field in inventory.
Alternatives: pessimistic locking (slower), saga (overkill).
```

### Rules

1. **Write in your own language.** Russian, English, mixed - it does not matter.
2. **Do not formalize.** No JSON Schema, no templates. Free text.
3. **More context is better than less.** Include the DBA conversation, a Figma screenshot, or an excerpt from the spec.
4. **Record decisions.** If you chose an approach, write down why. That is an ADR.
5. **Do not fear gaps.** If you do not know the item limit, say so. The LLM will move it into open questions.

### Phase 1 readiness checklist

- [ ] There is at least one file in `artifacts/` describing what to build
- [ ] It is clear who the user is and what they do (user flow)
- [ ] Known constraints are specified (latency, DB, security)
- [ ] Decisions that were made are recorded (if any)
- [ ] Unanswered questions are explicitly written down

**Time**: 15-60 minutes. Depends on task size.

---

## Phase 2: Formalization - the LLM generates, you confirm

**Who does it**: the LLM (via `/generate`), then you review.

```
/generate
```

The LLM reads all your artifacts and generates:

| Artifact | Where (inside milestone) | What is inside |
|----------|------------------------|-----------|
| Glossary | `human/glossary.yaml` (global) | Types, enums, domain terms |
| Contracts | `contracts/*.md` | Formal specifications of each operation |
| Contracts (YAML) | `contracts/*.yaml` | Machine-readable IR |
| Test specs | `test-specs/*.md` | What to test (not test code, but specifications) |
| Traceability | `traceability.yaml` | REQ -> Contract -> Test -> Gate chain |
| Plan | `plan.md` | Table of contents: scope, stages, budget |
| Stages | `stage_1.md`, `stage_2.md`, ... | Tasks for each stage |
| Open questions | `open-questions.md` | Questions the LLM could not answer |
| Stack | `project.yaml -> stack` (global) | Components, languages, dependencies |

### What you do after `/generate`

**Read the contracts.** They are the main artifact. Each contract looks like:

```markdown
# order.create v1.0.0

## Sources       <- where the information came from (links to your artifacts)
## Intent        <- what it does, for whom, in what context
## Input         <- input JSON Schema (YAML)
## Output        <- response JSON Schema (YAML)
## Errors        <- error table with codes and conditions
## Invariants    <- business rules that must ALWAYS hold
## Examples      <- concrete request->response examples (happy + error)
## Edge Cases    <- boundary cases and resolutions
## NFR           <- latency, availability, throughput
## Security      <- security rules
```

**Key review question**: *"Is this what I meant?"*

- Do Sources reference the right artifacts?
- Does Intent describe what you wanted?
- Do Errors cover all cases?
- Are Invariants really invariants? (always true, no exceptions)
- Are the Examples realistic?

**Answer the open questions.** The LLM will honestly tell you what it does not know:

```
Open Questions:
  - "Is there a limit on the number of items in an order?"
  - "How to calculate discounts - at order level or item level?"
```

Options:
- Answer -> the LLM updates the contract on the next `/generate`
- Defer -> does not block, but creates a warning
- Do not know -> leave it open; it blocks `/verify`

**Three ways to answer open questions:**

1. **`/questions`** - LLM skill. Walks through questions interactively, gives recommendations based on artifacts, you answer, the LLM writes them down.
2. **`hlv dashboard`** -> Questions tab. Navigation: `↑↓`, `a`/`Enter` to answer, `d` to defer.
3. **Manually.** Open `open-questions.md` inside the milestone, change `[ ]` to `[x]`, and add the answer.

**Edit if needed.** Contracts are markdown. Open them and edit them directly.

### Iteration

```
/generate -> /questions (or dashboard) -> /generate -> /verify -> edits -> /verify -> ok
```

Going through 2-3 iterations is normal.

**Time**: 30-90 minutes for review + edits.

---

## Phase 3: Verification - the machine checks

**Who does it**: `hlv check` (deterministic) + `/verify` (LLM semantics).

```bash
hlv check
```

What `hlv check` verifies:

```
▶ Project map        - project.yaml parses, paths exist
▶ Contracts          - sections, YAML blocks, links, glossary refs, examples
▶ Test specs         - derived_from, test IDs, gate refs
▶ Traceability       - no dangling references, REQ->CTR->TST->GATE chains
▶ Plan               - DAG without cycles, contract coverage
▶ Code traceability  - every error/invariant/constraint has an @hlv marker in code
▶ LLM map            - every entry from llm/map.yaml exists on disk

PASSED - 0 errors, 0 warnings
```

If `hlv check` is green, you run full verification:

```
/verify
```

`/verify` does all of the above plus LLM semantic analysis:
- Contracts do not contradict each other
- Test specs cover all invariants and errors
- The plan is realistic (tasks fit inside LLM context windows)
- Artifacts are fully covered by contracts

Result: `READY for /implement` (stage status becomes `verified`) or `NEEDS FIXES`.

### Iteration

```
hlv check -> fix -> hlv check -> /verify -> fix -> /verify -> READY
```

Convenient option: `hlv check --watch` watches files and rechecks on save.

**Time**: 5-20 minutes.

---

## Phase 4: Implementation - the LLM writes code

**Who does it**: LLM agents (via `/implement`).

```
/implement
```

The LLM reads the plan and executes tasks:

```
Group 1 (sequential):   Domain Types        -> src/domain/
Group 2 (parallel):     order.create        -> src/features/order_create/
                         order.cancel        -> src/features/order_cancel/
                         Global Constraints  -> src/middleware/
Group 3 (parallel):     Integration Tests   -> tests/integration/
                         Observability       -> src/observability/
Group 4 (parallel):     Performance Tuning
                         Security Review     -> validation/security-review.md
```

Agents work in parallel inside a group. Between groups - git commit.

**Code is an LLM artifact, not a human artifact.** Humans do not read code. The LLM writes linearly: input -> validation -> logic -> output -> errors. No layers (`controller`/`service`/`repository`), no abstractions for the future. Files may be called `01.rs`, `handler.rs`, anything - file names are arbitrary. Unit tests live in the same file as the code (`#[cfg(test)] mod tests`). A separate `tests/` directory is only for integration tests (cross-contract scenarios). Duplication across features is normal until it causes real problems.

**`llm/map.yaml` is the main navigator.** When creating a new file, the agent MUST add an entry to `llm/map.yaml` with a description sufficient to choose the file without opening it. The LLM finds code by descriptions in `map.yaml`, not by file names. `hlv check` verifies that all entries exist on disk.

**`@hlv` markers.** Every test must carry an `@hlv <ID>` marker pointing to an error code, invariant, or constraint rule from contracts. This provides 100% contract->code traceability. `hlv check` verifies that all IDs are covered. Constraint rules that have `check_command` are exempt — they are verified by their command, not by markers. Example:

```rust
// @ctx: validates stock availability check from order.create contract
// @hlv OUT_OF_STOCK
#[cfg(test)]
mod tests {
    #[test]
    fn test_out_of_stock_returns_409() { ... }

    // @hlv prepared_statements_only
    #[test]
    fn test_no_sql_injection() { ... }
}
```

**You do not write code.** You review the result.

**Time**: depends on complexity. For 2 contracts - 10-30 minutes.

---

## Phase 5: Validation - gates decide

**Who does it**: the LLM (via `/validate`). You do not run technical commands.

```
/validate
```

Runs 7 mandatory gates:

| Gate | What it checks | Threshold |
|------|--------------|-------|
| Contract tests | All scenarios from contracts | 100% pass |
| Property-based | Invariants across 10K+ generations | 0 counterexamples |
| Integration | Cross-contract scenarios | P0=100%, P1>=95% |
| Performance | Latency, throughput | `p99 <= NFR` |
| Security | SAST + DAST + audit | 0 Critical/High |
| Mutation | Mutation testing | >=70% score |
| Observability | Metrics, traces, logs | Everything is present |

### What `/validate` does automatically

- Installs missing tools (`hypothesis`, `locust`, `mutmut`, etc.)
- Runs all gates
- If a gate fails, it **does not ask you to fix something manually**, it:
  - Adds FIX tasks to the plan (remediation tasks)
  - Adds missing constraints if needed
  - Adds an open question if your decision is required

### Results

- **RELEASE APPROVED** -> everything passed, deploy
- **RELEASE BLOCKED** -> fix plan created -> `/implement` -> `/validate`
- **Decision needed** -> open question -> answer -> `/implement` -> `/validate`

### Remediation cycle

```
/validate -> BLOCKED
  ├─ FIX tasks added to the plan
  ├─ /implement (executes FIX tasks)
  └─ /validate (recheck)
```

You do not run `pip install` and you do not patch code manually. Agents handle that. You only make decisions if needed.

**Time**: 5-15 minutes (depends on tests).

---

## Full Picture

```
Phase 0          Phase 1           Phase 2          Phase 3        Phase 4         Phase 5
hlv init    ->   /artifacts   ->   /generate   ->   hlv check  ->  /implement  ->  /validate
                 or manual          + review         /verify        LLM code        gates
5 sec             15-60 min         30-90 min       5-20 min       10-30 min       5-15 min
                  HUMAN             LLM + you       machine        LLM             machine
                  provides          iterate         verifies       writes          proves
                  context
```

Total time from zero to production-ready code with proof: **1-3 hours** for a typical microservice with 2-5 contracts.

---

## Git Workflow

### Branch strategy

When `branch_per_milestone: true` in `project.yaml -> git_policy`, each milestone gets its own git branch. `hlv milestone new "feature"` automatically creates branch `{branch_prefix}{feature}` (by default `feature/feature`) and switches to it. `hlv milestone done` merges according to `merge_strategy` (`squash` / `merge` / `rebase`).

### Commit convention

`git_policy.commit_convention` defines the commit message format:

| Convention | Format | Example |
|-----------|--------|--------|
| `conventional` | `type(scope): description` | `feat(001-order): add stock validation` |
| `simple` | `[scope] description` | `[001-order] add stock validation` |
| `custom` | from `custom_template` | arbitrary |

### Using `hlv commit-msg`

```bash
hlv commit-msg                       # message according to the project convention
hlv commit-msg --type fix            # commit type (for conventional)
hlv commit-msg --stage               # include stage number in the message
```

The command determines the current milestone and stage from `milestones.yaml`, applies the convention (global or milestone override from `MilestoneGitConfig`), and prints a ready-to-use message. It can be used in a git hook: `git commit -m "$(hlv commit-msg)"`.

---

## Dashboard

### Constraints tab

`hlv dashboard` includes a Constraints tab that displays all constraint files from `human/constraints/`. For each file it shows: owner, rule count, severity distribution. Navigation: `↑↓` selects a file, `Enter` expands rules.

### Gates CRUD in Dashboard

The Gates tab in the dashboard allows interactive gate management:

| Key | Action |
|---------|----------|
| `a` | Add a new gate (interactive form) |
| `d` | Remove the selected gate |
| `e` | Enable / disable the gate (toggle `enabled`) |
| `c` | Set the command for the selected gate |
| `r` | Run the selected gate |
| `R` | Run all enabled gates |

Run results are displayed inline: exit code, runtime, first lines of stdout/stderr.

---

## FAQ

### What if I want to add a feature to an existing project?

1. `hlv milestone new "my-feature"` - creates a new milestone + branch
2. `/artifacts` -> `/generate` -> `/verify` -> `/implement` (by stages) -> `/validate`
3. `hlv milestone done` - merge

### What if the contract is wrong?

Edit the markdown directly. A contract is just a regular `.md` file. After editing:

```bash
hlv check          # quick check
/verify            # full verification
```

### What if implemented code needs to change?

1. Update the contract
2. `/verify` - checks that everything is aligned
3. `/implement` - regenerates the affected tasks
4. `/validate` - proves behavior is intact through the equivalence policy

### What if `hlv check` shows warnings?

Warnings do not block. Errors do block. But warnings are still worth fixing.

### Can I write code manually instead of using `/implement`?

Yes. HLV does not force code generation. Contracts + validation specs are useful on their own as a specification and test plan. Write the code manually, then `/validate` will check it through the same gates.

### What if `hlv check` shows warnings that are too early to fix?

`hlv check` takes the current phase into account (stage status in `milestones.yaml`). Warnings expected at the current phase are automatically downgraded to info. For example, "no gates mapped" is normal before the `validating` phase.

### What should I do with open questions I do not know how to answer?

Three options:
- **Find out and answer** - best option. Use `/questions` (LLM-recommended) or `hlv dashboard` -> Questions (answer manually)
- **Defer** - postpone it; does not block `check`, but produces a warning
- **Leave it open** - blocks `/verify`; it must be resolved before implementation

### What is the minimum set of artifacts needed to start?

One file in `tasks/` describing what to build. That is enough for `/generate`. More context gives better contracts, but you can start with the minimum.

---

## Cheatsheet

```bash
# Bootstrap (init creates the first milestone automatically)
hlv init --project my-service --owner my-team --profile standard

# Next milestones
hlv milestone new "feature"  # create a milestone + branch
hlv milestone status         # current milestone + stages
hlv milestone list           # all milestones (current + history)
hlv milestone done           # merge milestone
hlv milestone abort          # abort milestone

# Context dump
/artifacts                   # interactive interview

# Generation
/generate                    # artifacts -> contracts + validation + stages

# Verification
hlv check                    # structural validation + run gate commands + constraint checks (CST-050)
hlv check --watch            # same + watch
/verify                      # full verification (structure + semantics)

# Overview
hlv status                   # milestone + stages
hlv plan --visual            # stages + tasks
hlv trace --visual           # REQ->CTR->TST->GATE chain
hlv gates                    # gate status (enabled, commands, cwd)
hlv workflow                 # next actions for the current stage
hlv dashboard                # interactive TUI (Gates tab = management)

# Gate management
hlv gates enable <ID>        # enable gate
hlv gates disable <ID>       # disable gate
hlv gates set-cmd <ID> "cmd" # set command
hlv gates set-cwd <ID> "dir" # set working directory
hlv gates run                # run all gates with commands

# Task management (task-level tracking)
hlv task list [--stage N] [--status S] [--label L] [--json]
hlv task start <TASK-ID>     # checks dependencies, sets in_progress
hlv task done <TASK-ID>      # marks task as done
hlv task block <ID> --reason "..."   # manual block
hlv task unblock <ID>        # remove block
hlv task status [--json]     # summary across stages
hlv task add <ID> <name> --stage <N> [--description "..."]  # add task (auto-reopens stage)
hlv task sync [--force]      # sync with stage_N.md
hlv task label <ID> add|remove <label>
hlv task meta <ID> set|delete <key> [<value>]

# Stage management
hlv stage reopen <N>            # revert: implemented→implementing, validated→validating
hlv stage label <N> add|remove <label>
hlv stage meta <N> set|delete <key> [<value>]
hlv milestone label add|remove <label>
hlv milestone meta set|delete <key> [<value>]

# Constraint checks
hlv constraints check                    # run check_command for all rules
hlv constraints check observability      # filter by constraint
hlv constraints check --rule my_rule     # filter by rule
hlv constraints check --json             # JSON output

# Artifacts and glossary
hlv artifacts [--global|--milestone] [--json]
hlv artifacts show <name> [--json]
hlv glossary [--json]

# JSON output for automation
hlv status --json
hlv plan --json
hlv check --json
hlv trace --json
hlv workflow --json

# Implementation
/implement                   # stage/plan -> code + tests + gate commands

# Validation
/validate                    # gates -> release decision
```
