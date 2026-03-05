# HLV Roadmap

## Phase 1: CLI Binary - `hlv` ✅

Cross-platform Rust binary. Compiler and validator for an HLV project.

**Status: implemented in v0.1.0**

### Commands

| Command | Status | What it does |
|---------|--------|-----------|
| `hlv init` | ✅ | Scaffold a new HLV project: directories, policy templates, `project.yaml` |
| `hlv check` | ✅ | Structural validation: parsing, references, sections, dependency DAG |
| `hlv check --watch` | ✅ | File watcher with automatic revalidation on changes |
| `hlv status` | ✅ | Project state: phase, contracts, tasks, gates, open questions |
| `hlv plan` | ✅ | Output the plan: tasks, dependencies, parallel groups |
| `hlv plan --visual` | ✅ | ASCII dependency graph with boxes and arrows |
| `hlv trace` | ✅ | Traceability: requirement -> contract -> test -> gate |
| `hlv trace --visual` | ✅ | Visual traceability table |
| `hlv gates` | ✅ | Gate state: passed/not passed, thresholds, mandatory/optional |

### `hlv check` - deterministic checks

Code traceability:
- [x] `@hlv <ID>` markers: every contract error, invariant, and constraint rule is traced to test code
- [x] Automatic ID collection from `contracts/*.yaml` (errors, invariants) and `constraints/*.yaml` (rules)
- [x] Scan `src/` and `tests/` for `@hlv` markers
- [x] Phase-aware: at `implementing`, missing markers = info; at `implemented`+ = warning

Contracts:
- [x] Every `.md` in `human/contracts/` has a `# <id> v<semver>` header
- [x] Required sections: Sources, Intent, Input, Output, Errors, Invariants, Examples, NFR, Security
- [x] YAML blocks inside MD parse without errors
- [x] Links in Sources point to existing files in `artifacts/`
- [x] Types in Input/Output resolve through `glossary.yaml`
- [x] At least 1 happy-path Example and 1 error Example

Validation:
- [x] Every test spec has `derived_from` pointing to an existing contract
- [x] Every test ID is unique
- [x] Every test is mapped to a gate
- [x] There is a property-based test for every invariant
- [x] There is a contract test for every error

Traceability:
- [x] No dangling references (requirement -> contract -> test)
- [x] Every REQ -> at least 1 contract -> at least 1 test -> at least 1 gate
- [x] Warning for uncovered artifacts

Plan:
- [x] Dependency graph without cycles (petgraph)
- [x] Every contract is covered by at least one task
- [x] Parallel groups are checked for conflicts

Project map:
- [x] `project.yaml` parses
- [x] All paths in `paths` exist
- [x] `glossary_types` match `glossary.yaml`
- [x] `open_questions` reference valid contracts

LLM map (`llm/map.yaml`):
- [x] Forward check (MAP-010): every entry exists on disk
- [x] Reverse check (MAP-020): every file on disk is listed in the map
- [x] Ignore patterns: glob patterns exclude build artifacts from reverse check

### Exit codes

| Code | Meaning |
|------|---------|
| 0 | Everything is valid |
| 1 | There are warnings |
| 2 | There are errors |

### Stack

- Rust, edition 2021
- `clap` - CLI
- `serde` + `serde_yaml` - YAML parsing
- `pulldown-cmark` - Markdown parsing
- `petgraph` - dependency graph validation
- `colored` - colored output
- No external runtimes, no network - pure offline binary

---

## Phase 2: TUI - visualization ✅

Interactive terminal interface on top of the same binary.

**Status: implemented in v0.1.0**

### `hlv dashboard`

| Tab | Contents |
|-----|-----------|
| Status | Project, status, last skill, `updated_at` |
| Contracts | Contract table: id, version, status, owner, dependencies |
| Plan | Task groups with status icons and assigned agents |
| Gates | Gate table: id, type, mandatory, enabled, cwd, command. Controls: `e` on/off, `c` command, `w` cwd, `x` clear cmd |
| Questions | Open questions: answer (`a`/`Enter`), defer (`d`), navigation |

Controls: `q` quit, `Tab` switch, `↑↓` scroll, `r` reload, `a`/`Enter` answer (Questions), `d` defer, `e`/`c`/`w`/`x` (Gates)

### Stack (Phase 2)

- `ratatui` - TUI framework
- `crossterm` - terminal backend
- `notify` - file watching for `--watch`

---

## Phase 3: Milestone Architecture ✅

Two-level model for incremental work.

**Status: implemented**

### Data model
- `milestones.yaml` - tracker for the current milestone + history
- `model::milestone::MilestoneMap` - Rust struct
- `model::stage::StagePlan` - `stage_N.md` parser
- `schema/milestones-schema.json` - JSON Schema

### CLI commands
| Command | What it does |
|---------|-----------|
| `hlv milestone new` | Create a milestone + branch, auto-increment the number |
| `hlv milestone status` | Current milestone + stages |
| `hlv milestone list` | Current + history |
| `hlv milestone done` | Merge and move to history |
| `hlv milestone abort` | Abort the current milestone |

### Adapting existing commands
All commands (`check`, `status`, `workflow`, `plan`, `trace`, `gates`, `dashboard`) automatically detect milestone mode and operate on the current milestone.

### Skills
All 6 skills were updated for milestone mode:
- `/artifacts` -> output to `human/milestones/{id}/artifacts/`
- `/generate` -> stage decomposition, `plan.md` + `stage_N.md`
- `/verify` -> milestone contracts vs global glossary/constraints
- `/implement` -> stage-aware execution, reads `stage_N.md`
- `/validate` -> two-phase (`milestone` gates -> global scenarios)
- `/questions` -> per-milestone `open-questions.md`

---

## Phase 4: Extensions (planned)

| Feature | Description |
|------|---------|
| `hlv diff` | Contract version comparison, semantic diff |
| `hlv export --json` | Export data to JSON for CI/CD integrations |
| Cross-compilation | Binaries for Linux, Windows, macOS (arm64 + x86) |
| CI integration | GitHub Actions: `hlv check` in the pipeline |
| `/verify` integration | ✅ Replace the bash script with `hlv check` inside the `/verify` skill |
| `hlv watch --dashboard` | TUI dashboard with live updates when files change |
