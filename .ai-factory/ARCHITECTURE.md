# Architecture: Structured Modules (Technical Layer)

## Overview

HLV is a compiled Rust command-line tool with MCP server support, a terminal UI,
schema-backed project models, and deterministic validators. The architecture is a
structured modular monolith: each top-level Rust module owns one technical
capability, while shared data contracts live in `model` and shared parsing or
utility code lives in narrow support modules.

This pattern fits the project because HLV ships as one binary, has strong module
boundaries, and needs predictable behavior more than independently deployable
services. The design should stay easy for agents and maintainers to navigate:
commands orchestrate, models represent persisted files, checks emit diagnostics,
and integration surfaces reuse those same core functions.

## Decision Rationale

- **Project type:** Rust CLI plus MCP server and TUI for contract-driven
  development workflows.
- **Tech stack:** Rust 2021, `clap`, `serde`, `schemars`, `tokio`, `rmcp`,
  `ratatui`, tree-sitter language parsers, and integration tests.
- **Key factor:** HLV is a single deterministic binary, so module clarity and
  strict data-flow boundaries matter more than distributed-service boundaries.

## Folder Structure

```text
src/
  main.rs              # CLI argument parsing and top-level command dispatch
  lib.rs               # shared project-root/context helpers and module exports

  cmd/                 # command orchestration and user-facing CLI behavior
    check.rs
    init.rs
    index.rs
    milestone.rs
    ...

  check/               # deterministic validation domains and diagnostics
    project_map.rs
    llm_map.rs
    code_trace.rs
    sec_markers.rs
    index.rs
    ...

  model/               # serialized project formats and in-memory data models
    project.rs
    milestone.rs
    llm_map.rs
    index.rs
    policy.rs
    ...

  index/               # signature-index extraction and build logic
    builder.rs
    languages.rs
    mod.rs

  mcp/                 # MCP resources, tools, routing, watcher, workspace mode
    mod.rs
    resources.rs
    tools.rs
    router.rs
    watcher.rs
    workspace.rs

  tui/                 # terminal dashboard state and rendering
    app.rs
    tabs/
    widgets.rs

  parse/               # reusable parsers for markdown and structured text
  util/                # small reusable helpers with no project-domain ownership

schema/                # JSON Schemas for persisted YAML/JSON artifacts
skills/                # agent skill instructions shipped with HLV
docs/                  # user and maintainer documentation
tests/                 # integration tests and fixtures
```

## Dependency Rules

Dependencies should follow this direction:

```text
main -> cmd -> model/check/index/mcp/tui helpers
cmd  -> model + check + index + util + parse
check -> model + parse + util
mcp  -> cmd/model/check/index through public functions only
tui  -> model/cmd helpers for loading and saving project state
model -> serde/schemars/std only, plus narrowly justified parsing helpers
```

- Allowed: `cmd` modules load models, call validators, run gate commands, and
  format user-facing results.
- Allowed: `check` modules read model structs and return `Vec<Diagnostic>`
  without printing or mutating project files.
- Allowed: `mcp` tools call the same command/model functions as the CLI so JSON
  and MCP behavior stay consistent.
- Forbidden: `model` importing `cmd`, `check`, `mcp`, or `tui`.
- Forbidden: validators writing files, printing directly, or depending on TUI/MCP
  concerns.
- Forbidden: adding a schema field without updating model structs, validation,
  tests, skills, and docs in the same change.

## Layer/Module Communication

- CLI entrypoint code in `main.rs` resolves the repository root once and passes it
  to command modules. Commands decide whether they need repository paths or HLV
  config paths through `ProjectContext` and `config_root`.
- Commands communicate with persisted project state through `model::*` load/save
  APIs. Avoid ad hoc YAML parsing outside model types unless the file is not a
  modeled artifact.
- Validators communicate only through `Diagnostic` values. A diagnostic code
  prefix belongs to one validation domain and must be registered in `AGENTS.md`
  and `src/cmd/explain.rs`.
- MCP resources and tools should be thin adapters over existing model/command
  behavior. They must not invent alternate validation, indexing, or lifecycle
  semantics.
- The TUI may keep UI state, but project data should still be loaded and saved
  through the same model paths used by CLI commands.

## Key Principles

1. **Deterministic core behavior.** HLV does not rely on LLM calls at runtime.
   Validation, indexing, schema checks, and lifecycle commands must be repeatable.
2. **One persisted schema, one model owner.** YAML/JSON artifacts have typed Rust
   models and schema coverage. Unknown fields should be rejected where the
   format is intended to be strict.
3. **Diagnostics are contracts.** Diagnostic codes are user-facing API. New codes
   need tests, explanation text, and registry updates.
4. **Command modules orchestrate; model and check modules decide.** Keep business
   rules and structural validation out of CLI formatting code when a reusable
   model/check function can own them.
5. **Adopted and greenfield layouts share one path abstraction.** Do not scatter
   `root.join("project.yaml")` style assumptions. Use `ProjectContext`,
   `config_root`, `repo_path`, and `hlv_path` consistently.

## Code Organization Note

- **New features:** Add code to the module that owns the behavior. For example,
  new validation domains go in `src/check`, command entry points go in `src/cmd`,
  persisted file structs go in `src/model`, and MCP exposure goes in `src/mcp`.
- **Existing code:** The current technical-module structure is the architecture.
  Do not introduce a parallel DDD, clean-architecture, or microservice folder tree
  for isolated features.
- **Interoperability:** When a new surface exposes existing behavior, keep it as
  an adapter. For example, an MCP tool should call the same lower-level operation
  as the CLI command instead of duplicating command semantics.

## Code Examples

### Command Orchestration

```rust
pub fn run(project_root: &Path, json: bool) -> anyhow::Result<()> {
    let config_root = hlv::config_root(project_root);
    let project = ProjectMap::load(&config_root.join("project.yaml"))?;
    let diagnostics = check::project_map::check_project_map(&config_root, &project);

    if json {
        print_json(&diagnostics)?;
    } else {
        print_human_report(&diagnostics);
    }

    Ok(())
}
```

Command code may load models, call validators, and format output. It should not
embed schema-specific validation rules that belong in `src/check`.

### Validation Domain

```rust
pub fn check_example(project: &ProjectMap) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    if project.project.trim().is_empty() {
        diags.push(Diagnostic::error(
            "PRJ-001",
            "project name must not be empty",
        ));
    }

    diags
}
```

Validators return diagnostics and leave policy decisions such as strictness,
waivers, JSON formatting, and exit codes to the shared check/report pipeline.

### Path Resolution

```rust
let context = ProjectContext::from_root(project_root);
let project_yaml = context.hlv_path("project.yaml");
let observed_source = context.repo_path("src");
```

Use explicit repository-vs-HLV path helpers for code that must work in both
greenfield and adopted projects.

## Anti-Patterns

- Do not add a persisted field only in `src/model`; schema, checks, tests, skills,
  and documentation must change together.
- Do not duplicate CLI behavior inside MCP tools or TUI code.
- Do not let `model` depend on command, TUI, MCP, or validation modules.
- Do not make validators mutate files or print directly.
- Do not bypass `ProjectContext` for HLV-owned paths in code that should support
  adopted projects.
- Do not add future-proof abstractions unless they remove concrete complexity in
  the current codebase.
