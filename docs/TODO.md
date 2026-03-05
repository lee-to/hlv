[x] - define how a person starts a project -> free-form `artifacts/`
[x] - define how a person writes contracts -> LLM generates, person confirms
[x] - define the contract format -> MD + embedded YAML blocks
[x] - define that validation is generated from contracts -> `/generate` creates test-specs, scenarios, traceability
[x] - package the workflow as skills -> `/generate`, `/verify`
[x] - plan with parallel agents -> parallel groups in `plan.md`
[x] - implement structural validation for `/verify` (Step 1) -> `hlv check`
[x] - add `/implement` skill -> `skills/implement/SKILL.md`
[x] - add `/validate` skill (gate execution) -> `skills/validate/SKILL.md`
[x] - `order.cancel.md` example in the new format -> `human/contracts/order.cancel.md`
[x] - integrate with Handoff for agent orchestration -> README section 9, `/implement` skill
[x] - template for initializing a new HLV project (`/init` skill) -> `skills/init/SKILL.md`
[x] - cross-platform Rust binary for validation and TUI -> `hlv v0.1.0`

## Implemented in `hlv v0.1.0`

[x] - `Cargo.toml` + CLI skeleton (clap derive) -> `src/main.rs`
[x] - Serde models for all YAML files (12 types) -> `src/model/`
[x] - Markdown parser (pulldown-cmark) -> `src/parse/markdown.rs`
[x] - `ContractMd` parser from `.md` files -> `src/model/contract_md.rs`
[x] - `hlv init --project --owner` -> project scaffold
[x] - `hlv check` -> validation for contracts, test specs, traceability, plan, project map
[x] - `hlv check --watch` -> file watcher with automatic revalidation
[x] - `hlv status` -> project summary
[x] - `hlv plan [--visual]` -> ASCII dependency graph
[x] - `hlv trace [--visual]` -> REQ->CTR->TST->GATE table
[x] - `hlv gates` -> gate status
[x] - `hlv dashboard` -> interactive TUI (ratatui, 5 tabs)
[x] - `Makefile` (build, test, clippy, install)
[x] - 19 tests (unit + integration), 0 clippy warnings

## Implemented after `v0.1.0`

[x] - integrate `hlv check` into the `/verify` skill instead of a bash script
[x] - remove legacy bash scripts (`check.sh`, `verify.sh`)
[x] - phase-aware check: warnings are downgraded to info in early phases
[x] - contract version alignment (MD vs YAML vs `project.yaml`)
[x] - task-level `depends_on` in plan DAG validation
[x] - traceability path from `project.yaml` instead of hardcoded
[x] - section-based parsing of YAML blocks in contracts (Input/Output/NFR)
[x] - `/questions` skill - interactive resolution of open questions with recommendations
[x] - `hlv dashboard` Questions tab - answer/defer directly from the TUI
[x] - `hlv init` adds a schema reference to an existing `project.yaml`
[x] - `@hlv` markers: code traceability - contract errors, invariants, constraint rules -> `@hlv <ID>` in tests
[x] - `hlv check` -> Code traceability section: auto-ID collection + code scan + CTR-010 warnings
[x] - phase-aware downgrade of CTR-010 until the implemented phase
[x] - `/implement` skill: TDD approach + required `@hlv` markers
[x] - `map.yaml` ignore patterns: glob patterns to exclude build artifacts from reverse check (MAP-020)
[x] - `hlv init`: default ignore patterns in the `map.yaml` template (`__pycache__`, `node_modules`, `target`, `dist`, etc.)
[x] - `/validate` remediation loop: diagnostics + FIX tasks instead of self-repair
[x] - `/implement` accepts `validating` status to execute remediation tasks
[x] - `/generate`: gates coverage check (Step 4b) - every gate covered by contracts/constraints
[x] - `/verify`: gates-to-contracts cross-check (Step 1g) - CRITICAL if a gate has no coverage
[x] - `hlv workflow`: remediation-aware guidance for `validating` status
[x] - dashboard plan tab: colored task status icons (fix white square)

## Milestone Architecture

[x] - `MilestoneMap` model (`milestones.yaml`, stages, history)
[x] - `StagePlan` parser (`stage_N.md`: tasks, deps, remediation)
[x] - `milestones-schema.json`
[x] - `hlv milestone new/status/list/done/abort` commands
[x] - `hlv init`: scaffold `milestones.yaml` + `human/milestones/`
[x] - dual-mode commands: check, status, workflow, plan, trace, gates, dashboard
[x] - TUI: milestone view (status, contracts, plan tabs)
[x] - Skills: all 6 `SKILL.md` files updated for milestone mode
[x] - Documentation: `ARCH.md`, `WORKFLOW.md`, `SPECS.md`, `ROADMAP.md`
[x] - Migration cleanup: removed serde aliases, custom deserializers, migrate functions
[x] - 12 milestone integration tests

## PLAN_PREPARE_MCP - preparation for MCP

[x] - P1.1: `TaskStatus` + `TaskTracker` model (`src/model/task.rs`)
[x] - P1.2: labels/meta on `MilestoneCurrent`
[x] - P1.3: tasks/labels/meta on `StageEntry` (persisted in `milestones.yaml`)
[x] - P1.4: `hlv task list/start/done/block/unblock/status/sync/label/meta` commands
[x] - P1.4: `hlv stage label/meta` commands
[x] - P1.4: `hlv milestone label/meta` commands
[x] - P1.5: automatic statuses (dependency check on start, auto stage->implementing)
[x] - P1.6: `hlv task sync` - synchronization with `stage_N.md` (safe delete, `--force`)
[x] - P2.1-P2.3: `ArtifactIndex`/`ArtifactFull` model + `hlv artifacts [show] [--json]`
[x] - P3: `--json` for status, plan, trace, check, workflow
[x] - P3: `hlv glossary [--json]` - new command
[x] - P4: `milestones-schema.json` updated (`TaskTracker`, `TaskStatus`, labels/meta)
[x] - 389 tests (171 unit + 218 integration), 0 warnings

## Next Steps

[ ] - cross-compilation (linux, windows)
[ ] - CI: GitHub Actions for `cargo test` + clippy + bin
