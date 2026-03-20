use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

use include_dir::{include_dir, Dir};

use super::style;
use crate::model::project::ProjectMap;

static EMBEDDED_SKILLS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/skills");

/// Gate profile determines which validation gates are created.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateProfile {
    /// contract_tests + security — CLI, TUI, scripts, prototypes
    Minimal,
    /// + integration_tests, property_based_tests — services, libraries
    Standard,
    /// + performance, mutation_testing, observability — production APIs, payment systems
    Full,
}

impl GateProfile {
    pub fn from_str_opt(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "minimal" | "min" => Ok(Self::Minimal),
            "standard" | "std" => Ok(Self::Standard),
            "full" => Ok(Self::Full),
            _ => anyhow::bail!(
                "Unknown gate profile: '{}'. Choose: minimal, standard, full",
                s
            ),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Standard => "standard",
            Self::Full => "full",
        }
    }
}

pub fn run(
    path: &str,
    project: Option<&str>,
    owner: Option<&str>,
    agent: Option<&str>,
    profile: Option<&str>,
) -> Result<()> {
    run_with_milestone(path, project, owner, agent, None, profile)
}

pub fn run_with_milestone(
    path: &str,
    project: Option<&str>,
    owner: Option<&str>,
    agent: Option<&str>,
    milestone: Option<&str>,
    profile: Option<&str>,
) -> Result<()> {
    let root = Path::new(path);
    let is_reinit = root.join("project.yaml").exists();

    // Embedded schemas — all go into schema/ directory
    let schemas: &[(&str, &str)] = &[
        (
            "schema/project-schema.json",
            include_str!("../../schema/project-schema.json"),
        ),
        (
            "schema/glossary-schema.json",
            include_str!("../../schema/glossary-schema.json"),
        ),
        (
            "schema/gates-policy-schema.json",
            include_str!("../../schema/gates-policy-schema.json"),
        ),
        (
            "schema/equivalence-policy-schema.json",
            include_str!("../../schema/equivalence-policy-schema.json"),
        ),
        (
            "schema/traceability-policy-schema.json",
            include_str!("../../schema/traceability-policy-schema.json"),
        ),
        (
            "schema/ir-policy-schema.json",
            include_str!("../../schema/ir-policy-schema.json"),
        ),
        (
            "schema/adversarial-guardrails-schema.json",
            include_str!("../../schema/adversarial-guardrails-schema.json"),
        ),
        (
            "schema/security-constraints-schema.json",
            include_str!("../../schema/security-constraints-schema.json"),
        ),
        (
            "schema/performance-constraints-schema.json",
            include_str!("../../schema/performance-constraints-schema.json"),
        ),
        (
            "schema/llm-map-schema.json",
            include_str!("../../schema/llm-map-schema.json"),
        ),
        (
            "schema/traceability-schema.json",
            include_str!("../../schema/traceability-schema.json"),
        ),
        (
            "schema/contract-schema.json",
            include_str!("../../schema/contract-schema.json"),
        ),
        (
            "schema/constraint-schema.json",
            include_str!("../../schema/constraint-schema.json"),
        ),
        (
            "schema/milestones-schema.json",
            include_str!("../../schema/milestones-schema.json"),
        ),
    ];

    if is_reinit {
        // Read project name from existing project.yaml
        let project_name = if let Some(p) = project {
            p.to_string()
        } else {
            let yaml = fs::read_to_string(root.join("project.yaml"))?;
            let pm: ProjectMap =
                serde_yaml::from_str(&yaml).context("failed to parse project.yaml")?;
            pm.project
        };

        // Detect agent from existing .{agent}/skills/ directory
        let agent_name = if let Some(a) = agent {
            a.to_string()
        } else {
            detect_agent(root)?
        };

        let agent_dir = format!(".{agent_name}");
        let skills_dir = format!("{agent_dir}/skills");

        style::header("init");
        style::hint(&format!(
            "project.yaml exists — updating skills and HLV.md (agent: {})",
            agent_name.bold(),
        ));

        // Update embedded skill tree (overwrite if content changed, add new files)
        for file in all_files(&EMBEDDED_SKILLS) {
            let rel = file.path().to_string_lossy().replace('\\', "/");
            let content = file.contents_utf8().unwrap_or("");
            write_or_update(root, &format!("{skills_dir}/{rel}"), content)?;
        }

        // HLV.md is always updated (generated, not user-editable)
        write_or_update(
            root,
            "HLV.md",
            &hlv_template(&project_name, &agent_name, &skills_dir),
        )?;

        // Schemas are always updated
        for (path, content) in schemas {
            write_or_update(root, path, content)?;
        }

        // Ensure all YAML files have $schema comments
        ensure_project_yaml_schema(root)?;
        ensure_yaml_schemas(root)?;

        // AGENTS.md is user-owned — skip if exists
        write_template(root, "AGENTS.md", &agents_template(&project_name))?;

        println!();
        style::ok("Skills and HLV.md updated");
        return Ok(());
    }

    // Fresh init — resolve missing args interactively
    let project_name = match project {
        Some(p) => p.to_string(),
        None => prompt("Project name")?,
    };
    let owner_name = match owner {
        Some(o) => o.to_string(),
        None => prompt("Owner (team or person)")?,
    };
    let agent_name = match agent {
        Some(a) => a.to_string(),
        None => prompt_with_default("Agent name", "claude")?,
    };

    let gate_profile = match profile {
        Some(p) => GateProfile::from_str_opt(p)?,
        None => prompt_gate_profile()?,
    };

    let milestone_name = match milestone {
        Some(m) => m.to_string(),
        None => prompt_with_default("First milestone name", "init")?,
    };

    let linear_arch = prompt_yes_no("Enable linear architecture style?", true)?;
    let hlv_markers = prompt_yes_no("Enable @hlv code traceability markers?", true)?;

    tracing::debug!(
        linear_architecture = linear_arch,
        hlv_markers = hlv_markers,
        "Feature flags selected"
    );

    let agent_dir = format!(".{agent_name}");
    let skills_dir = format!("{agent_dir}/skills");

    style::header("init");
    println!(
        "  Initializing: {} ({}) agent: {} profile: {}",
        project_name.bold(),
        owner_name.dimmed(),
        agent_name.bold(),
        gate_profile.label().bold(),
    );

    // Create directories
    let dirs = vec![
        "human/artifacts".to_string(),
        "human/constraints".to_string(),
        "human/milestones".to_string(),
        "schema".to_string(),
        "validation/test-specs".to_string(),
        "validation/scenarios".to_string(),
        "llm/src".to_string(),
        "llm/tests".to_string(),
    ];
    for d in &dirs {
        let dir = root.join(d);
        fs::create_dir_all(&dir)?;
        style::file_op("mkdir", d, None);
    }

    // Create template files
    write_template(
        root,
        "human/glossary.yaml",
        &glossary_template(&project_name),
    )?;
    write_template(
        root,
        "human/constraints/security.yaml",
        &security_template(&owner_name),
    )?;
    write_template(
        root,
        "human/constraints/performance.yaml",
        &performance_template(&owner_name),
    )?;
    write_template(
        root,
        "human/constraints/observability.yaml",
        &observability_template(&owner_name),
    )?;
    write_template(
        root,
        "validation/gates-policy.yaml",
        &gates_policy_template(gate_profile),
    )?;
    write_template(
        root,
        "validation/equivalence-policy.yaml",
        EQUIV_POLICY_TEMPLATE,
    )?;
    write_template(
        root,
        "validation/traceability-policy.yaml",
        TRACE_POLICY_TEMPLATE,
    )?;
    write_template(root, "validation/ir-policy.yaml", IR_POLICY_TEMPLATE)?;
    write_template(
        root,
        "validation/adversarial-guardrails.yaml",
        ADV_GUARDRAILS_TEMPLATE,
    )?;
    write_template(root, "llm/map.yaml", &llm_map_template())?;
    write_template(
        root,
        "human/traceability.yaml",
        &traceability_template(&owner_name),
    )?;
    write_template(
        root,
        "project.yaml",
        &project_template(&project_name, &owner_name, linear_arch, hlv_markers),
    )?;
    write_template(root, "milestones.yaml", &milestones_template(&project_name))?;
    write_template(
        root,
        "HLV.md",
        &hlv_template(&project_name, &agent_name, &skills_dir),
    )?;
    for (path, content) in schemas {
        write_template(root, path, content)?;
    }
    write_template(root, "AGENTS.md", &agents_template(&project_name))?;

    // Write embedded skill tree
    for file in all_files(&EMBEDDED_SKILLS) {
        let rel = file.path().to_string_lossy().replace('\\', "/");
        let content = file.contents_utf8().unwrap_or("");
        write_template(root, &format!("{skills_dir}/{rel}"), content)?;
    }

    // Create first milestone
    super::milestone::run_new(root, &milestone_name)?;

    println!();
    style::ok(&format!("Project scaffold created at {}", root.display()));
    println!();

    // Show workflow so the user immediately sees what to do next
    super::workflow::run(root, false)?;

    Ok(())
}

/// Prompt user for a value, reading from stdin.
fn prompt(label: &str) -> Result<String> {
    print!("  {} {}: ", "?".cyan().bold(), label);
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin()
        .lock()
        .read_line(&mut line)
        .context("failed to read input")?;
    let value = line.trim().to_string();
    anyhow::ensure!(!value.is_empty(), "{label} cannot be empty");
    Ok(value)
}

/// Prompt user with a default value shown in brackets.
fn prompt_with_default(label: &str, default: &str) -> Result<String> {
    print!("  {} {} [{}]: ", "?".cyan().bold(), label, default.dimmed());
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin()
        .lock()
        .read_line(&mut line)
        .context("failed to read input")?;
    let value = line.trim();
    if value.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(value.to_string())
    }
}

/// Prompt user for a yes/no answer with a default.
fn prompt_yes_no(label: &str, default: bool) -> Result<bool> {
    let hint = if default { "Y/n" } else { "y/N" };
    print!("  {} {} [{}]: ", "?".cyan().bold(), label, hint);
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin()
        .lock()
        .read_line(&mut line)
        .context("failed to read input")?;
    let value = line.trim().to_lowercase();
    if value.is_empty() {
        Ok(default)
    } else {
        match value.as_str() {
            "y" | "yes" => Ok(true),
            "n" | "no" => Ok(false),
            _ => anyhow::bail!("Expected y/n, got '{}'", line.trim()),
        }
    }
}

/// Detect agent name from existing `.{agent}/skills/` directory.
fn detect_agent(root: &Path) -> Result<String> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') && name.len() > 1 && entry.path().join("skills").is_dir() {
            return Ok(name[1..].to_string());
        }
    }
    anyhow::bail!(
        "Cannot detect agent: no .{{agent}}/skills/ directory found. Pass --agent explicitly."
    )
}

fn all_files<'a>(dir: &'a Dir<'a>) -> Vec<&'a include_dir::File<'a>> {
    let mut result = Vec::new();
    collect_dir_files(dir, &mut result);
    result
}

fn collect_dir_files<'a>(dir: &'a Dir<'a>, out: &mut Vec<&'a include_dir::File<'a>>) {
    for entry in dir.entries() {
        match entry {
            include_dir::DirEntry::File(f) => out.push(f),
            include_dir::DirEntry::Dir(d) => collect_dir_files(d, out),
        }
    }
}

fn write_template(root: &Path, rel: &str, content: &str) -> Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if path.exists() {
        style::file_op("skip", rel, Some("exists"));
    } else {
        fs::write(&path, content)?;
        style::file_op("create", rel, None);
    }
    Ok(())
}

/// Ensure project.yaml has the `$schema` comment and `spec:` field.
fn ensure_project_yaml_schema(root: &Path) -> Result<()> {
    let path = root.join("project.yaml");
    let mut content = fs::read_to_string(&path)?;
    let mut changed = false;

    // Add yaml-language-server $schema comment if missing
    if !content.contains("yaml-language-server: $schema=") {
        content = format!(
            "# yaml-language-server: $schema=schema/project-schema.json\n{}",
            content
        );
        changed = true;
    }

    // Add spec field if missing (after schema_version line)
    if !content.lines().any(|l| l.trim().starts_with("spec:")) {
        if let Some(pos) = content.find("\nschema_version:") {
            // Find end of schema_version line
            let after = &content[pos + 1..];
            if let Some(eol) = after.find('\n') {
                let insert_at = pos + 1 + eol + 1;
                content.insert_str(insert_at, "spec: schema/project-schema.json\n");
                changed = true;
            }
        }
    }

    if changed {
        fs::write(&path, &content)?;
        style::file_op("update", "project.yaml", Some("schema reference"));
    }

    Ok(())
}

/// Ensure all known YAML files have a $schema comment. Adds it if missing.
fn ensure_yaml_schemas(root: &Path) -> Result<()> {
    let known: &[(&str, &str)] = &[
        ("human/glossary.yaml", "../schema/glossary-schema.json"),
        (
            "human/constraints/security.yaml",
            "../../schema/security-constraints-schema.json",
        ),
        (
            "human/constraints/performance.yaml",
            "../../schema/performance-constraints-schema.json",
        ),
        (
            "human/constraints/observability.yaml",
            "../../schema/constraint-schema.json",
        ),
        (
            "validation/gates-policy.yaml",
            "../schema/gates-policy-schema.json",
        ),
        (
            "validation/equivalence-policy.yaml",
            "../schema/equivalence-policy-schema.json",
        ),
        (
            "validation/traceability-policy.yaml",
            "../schema/traceability-policy-schema.json",
        ),
        (
            "validation/ir-policy.yaml",
            "../schema/ir-policy-schema.json",
        ),
        (
            "validation/adversarial-guardrails.yaml",
            "../schema/adversarial-guardrails-schema.json",
        ),
        (
            "human/traceability.yaml",
            "../schema/traceability-schema.json",
        ),
        ("llm/map.yaml", "../schema/llm-map-schema.json"),
    ];

    for (rel, schema) in known {
        let path = root.join(rel);
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(&path)?;
        if content.contains("yaml-language-server: $schema=") {
            continue;
        }
        let updated = format!("# yaml-language-server: $schema={schema}\n{content}");
        fs::write(&path, updated)?;
        style::file_op("update", rel, Some("added $schema"));
    }

    Ok(())
}

fn write_or_update(root: &Path, rel: &str, content: &str) -> Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if path.exists() {
        let existing = fs::read_to_string(&path)?;
        if existing == content {
            style::file_op("skip", rel, Some("up to date"));
        } else {
            fs::write(&path, content)?;
            style::file_op("update", rel, None);
        }
    } else {
        fs::write(&path, content)?;
        style::file_op("create", rel, None);
    }
    Ok(())
}

fn glossary_template(project: &str) -> String {
    format!(
        r#"# yaml-language-server: $schema=../schema/glossary-schema.json
schema_version: 1
domain: {project}

types: {{}}

enums: {{}}

terms: {{}}

rules:
  - id: no_critical_synonyms
    description: "Critical entities must use canonical names in all contracts."
  - id: type_reuse_required
    description: "Contracts must reuse glossary types instead of redefining them."
"#
    )
}

fn security_template(owner: &str) -> String {
    format!(
        r#"# yaml-language-server: $schema=../../schema/security-constraints-schema.json
id: constraints.security.global
version: 1.0.0
owner: {owner}
intent: "Global security constraints for all contracts."

rules:
  - id: prepared_statements_only
    severity: critical
    statement: "All database access must use parameterized queries."
    enforcement: [sast, integration_test]
  - id: no_secrets_in_logs
    severity: critical
    statement: "Secrets, credentials, and tokens must never be logged."
    enforcement: [log_policy_check, runtime_scan]
  - id: authn_required
    severity: critical
    statement: "Authenticated identity is required for all state-changing endpoints."
    enforcement: [contract_test, integration_test]

exceptions:
  process: "Any exception requires security approval and expiry date."
  max_exception_days: 30
"#
    )
}

fn performance_template(owner: &str) -> String {
    format!(
        r#"# yaml-language-server: $schema=../../schema/performance-constraints-schema.json
id: constraints.performance.global
version: 1.0.0
owner: {owner}
intent: "Global performance envelopes and runtime budgets."

defaults:
  latency_p95_ms: 120
  latency_p99_ms: 250
  error_rate_max_percent: 0.5
  availability_slo: 99.9

overrides: []

validation:
  warmup_seconds: 30
  test_window_seconds: 300
  percentile_method: hdr_histogram
  fail_on_budget_exceed: true
"#
    )
}

fn observability_template(owner: &str) -> String {
    format!(
        r#"# yaml-language-server: $schema=../../schema/constraint-schema.json
id: constraints.observability.global
version: 1.0.0
owner: {owner}
intent: "Maximum observability — every operation is logged, every state change is traceable."

rules:
  - id: structured_logging_only
    severity: critical
    statement: "All log output must be structured (JSON / tracing spans). No println, dbg!, console.log without structure."
    enforcement: [sast, contract_test]
  - id: log_entry_exit
    severity: critical
    statement: "Every public endpoint / handler logs entry (parameters, request_id) and exit (result status, duration)."
    enforcement: [contract_test, integration_test]
  - id: log_all_errors
    severity: critical
    statement: "Every error path logs the error with full context: request_id, entity_id, input summary, error details."
    enforcement: [contract_test, runtime_scan]
  - id: log_state_changes
    severity: critical
    statement: "Every state mutation (DB write, status transition, cache invalidation) emits a log event with entity_id, old_state, new_state."
    enforcement: [contract_test, runtime_scan]
  - id: log_external_calls
    severity: critical
    statement: "Every outgoing call (HTTP, gRPC, DB query, queue publish) logs target, duration, and outcome."
    enforcement: [integration_test, runtime_scan]
  - id: request_correlation
    severity: critical
    statement: "All log events within a request carry a correlation ID (request_id / trace_id) propagated through the entire call chain."
    enforcement: [integration_test, runtime_scan]
  - id: no_sensitive_in_logs
    severity: critical
    statement: "PII, secrets, tokens, and passwords must be masked or excluded from all log output."
    enforcement: [log_policy_check, runtime_scan]
  - id: log_levels_correct
    severity: high
    statement: "Log levels used correctly: error for failures, warn for degraded, info for business events, debug for diagnostics. No info-level spam."
    enforcement: [contract_test]

exceptions:
  process: "Any exception requires team lead approval with justification and expiry date."
  max_exception_days: 14
"#
    )
}

fn project_template(
    project: &str,
    _owner: &str,
    linear_architecture: bool,
    hlv_markers: bool,
) -> String {
    format!(
        r#"# yaml-language-server: $schema=schema/project-schema.json
# HLV Project Map
schema_version: 1
project: {project}
status: draft
spec: schema/project-schema.json

paths:
  human:
    artifacts: human/artifacts/
    glossary: human/glossary.yaml
    constraints: human/constraints/
  validation:
    test_specs: validation/test-specs/
    scenarios: validation/scenarios/
    traceability: human/traceability.yaml
    gates_policy: validation/gates-policy.yaml
  llm:
    src: llm/src/
    tests: llm/tests/
    map: llm/map.yaml

constraints:
  - id: security.global
    path: human/constraints/security.yaml
    applies_to: all
  - id: performance.global
    path: human/constraints/performance.yaml
    applies_to: all
  - id: observability.global
    path: human/constraints/observability.yaml
    applies_to: all

features:
  linear_architecture: {linear_architecture}
  hlv_markers: {hlv_markers}

git:
  branch_per_milestone: false
  commit_convention: conventional
  commit_scopes:
    - feat
    - fix
    - refactor
    - test
    - docs
    - chore
  merge_strategy: manual

"#
    )
}

fn milestones_template(project: &str) -> String {
    format!(
        r#"# yaml-language-server: $schema=schema/milestones-schema.json
project: {project}
history: []
"#
    )
}

fn traceability_template(owner: &str) -> String {
    format!(
        r#"# yaml-language-server: $schema=../schema/traceability-schema.json
schema_version: 1
owner: {owner}
intent: "Requirement → contract → test → gate traceability map."
requirements: []
mappings: []

coverage_policy:
  require_full_traceability: true
  allow_unmapped_requirements: false
"#
    )
}

fn agents_template(project: &str) -> String {
    format!(
        r#"# AGENTS.md — Rules for LLM agents working on {project}

> **Read `HLV.md` first** — it contains the HLV methodology rules, project structure,
> skills reference, and agent protocol. This file is for project-specific rules.

---

## Project-specific rules

Add your team's conventions, coding standards, and project-specific constraints below.
This file is yours — `hlv init` will not overwrite it.

<!-- Example:
## Coding conventions
- Use snake_case for all identifiers
- All public functions must have doc comments
- Maximum function length: 50 lines

## Team rules
- All changes require review before merge
- No direct pushes to main branch
-->
"#
    )
}

fn hlv_template(project: &str, _agent: &str, skills_dir: &str) -> String {
    format!(
        r#"# HLV.md — HLV methodology rules for {project}

> This file is auto-generated by `hlv init`. Do not edit — it will be overwritten on update.
> For project-specific rules, edit `AGENTS.md`.

---

## 1. Entry Point

**Always start by reading `project.yaml`.** It is the single source of truth for:

- **status** — current project phase (`draft` → `implementing` → `implemented` → `validating` → `validated`)
- **paths** — where every file and directory lives
- **stack** — tech stack: components, languages, typed dependencies
- **validation** — state of each gate

Do NOT guess paths. Read them from `project.yaml`.

---

## 2. Iron Rules

### 2.1 Contracts are the source of truth

- Contracts live in `human/milestones/{{id}}/contracts/` (per milestone)
- Every line of code traces back to a contract
- If something isn't in a contract — it doesn't exist
- If a contract is wrong — fix the contract first, not the code

### 2.2 Code is a derived artifact

```
human artifacts → contracts → code
```

Code is third-order. Contracts are second-order. Human artifacts are first-order. You generate code FROM contracts, never the other way around.

### 2.3 One contract = one module

Each contract maps to exactly one directory. No cross-contract imports except through domain types. Duplication is preferred over coupling.

### 2.4 No changes without validation

After any change to contracts or code:
1. `hlv check` must pass (structural validation)
2. `/verify` must pass (semantic validation)
3. `/validate` must pass before release (all mandatory gates from gates-policy.yaml)

### 2.5 No invented behavior

If information is not in artifacts or contracts — do NOT invent it. Add an open question instead. Open questions block `/verify`.

### 2.6 Tests are proof — with `@hlv` traceability

Every test traces to a contract invariant, error case, or NFR. No "just in case" tests. Property-based tests for invariants (≥10,000 generations).

**Every test MUST carry an `@hlv <ID>` marker** linking it to a specific contract validation or constraint rule:

```
// @hlv OUT_OF_STOCK        ← contract error code
// @hlv atomicity            ← contract invariant
// @hlv prepared_statements_only  ← constraint rule
```

`hlv check` automatically collects all IDs from contracts (`errors[].code`, `invariants[].id`) and constraints (`rules[].id`), then scans `src/` and `tests/` to verify every ID has a marker. Missing markers are reported as `CTR-010` warnings.

### 2.7 Files are small and self-contained

Target: <300 lines per file, <10 files per feature. Each file does one thing. An LLM agent must understand any module in isolation.

### 2.8 Language selection is pragmatic

Prefer strict, compile-time-safe languages when they fit the problem. For backend/service/CLI/system components, start with strict, compile-time-safe languages that have good ecosystem fit.

This is guidance, not dogma. Do NOT force that preference onto UI/frontend, bots, automation, scripting, ML/data/AI-chain workloads, or SDK-centric integrations when TypeScript, Python, or another language is clearly the better fit. Python is not the default architectural preference, but it can be the right ecosystem-driven choice for ML and complex AI chains. If the fit is ambiguous — raise an open question instead of guessing.

### 2.9 No abstractions for the future

No base classes, generic frameworks, or plugin systems unless the contract explicitly requires extensibility. Simplest code that satisfies the contract.

### 2.10 Deterministic structure

Given the same contract, two different LLM agents MUST produce code with the same file layout and public API. The contract dictates structure.

### 2.11 Error paths are first-class

Every error from the contract's Errors table has an explicit code path. No catch-all handlers. No `unwrap()` / `expect()` in production code.

---

## 3. Where Things Live

```
project.yaml                 ← READ THIS FIRST
milestones.yaml              ← current milestone, stage, history
HLV.md                       ← HLV rules (this file, auto-generated)
AGENTS.md                    ← project-specific rules (user-editable)
human/
  artifacts/                 ← global project context (domain, stack, arch decisions)
  glossary.yaml              ← domain types (canonical, shared across milestones)
  constraints/*.yaml         ← security, performance rules (global)
  milestones/
    001-xxx/
      artifacts/             ← milestone-specific artifacts (features, unknowns)
      contracts/*.md         ← formal specifications
      contracts/*.yaml       ← machine-readable IR
      test-specs/*.md        ← what to test (derived from contracts)
      plan.md                ← milestone overview: scope, stages table
      stage_N.md             ← tasks for each stage
      traceability.yaml      ← REQ → CTR → TST → GATE
      open-questions.md      ← unresolved questions
validation/
  scenarios/*.md             ← cross-milestone integration scenarios
  gates-policy.yaml          ← gate thresholds
  equivalence-policy.yaml    ← regeneration rules
  traceability-policy.yaml   ← traceability rules
  ir-policy.yaml             ← IR versioning
  adversarial-guardrails.yaml
llm/
  map.yaml                   ← file map (hlv check validates all entries exist)
  src/                       ← generated code
```

---

## 4. Skills (Commands)

Skills live in `{skills_dir}/`. Each skill is a full prompt for a specific phase.

| Skill | What it does | When to use |
|-------|-------------|-------------|
| `/artifacts` | Interactive interview → fills milestone artifacts/ | At project start or when adding features |
| `/generate` | Artifacts → Contracts + Validation + Plan | After human adds artifacts |
| `/questions` | Interactive resolution of open questions | After /generate if open questions exist |
| `/verify` | Structural + semantic validation + gates coverage check | After /generate or manual edits |
| `/implement` | Plan → Code + Tests (executes plan tasks) | After /verify passes, or after /validate adds remediation tasks |
| `/validate` | Mandatory gates (from gates-policy.yaml) → Release decision or remediation plan | After /implement completes |

### Separation of concerns

Each skill has one job:
- **`/generate`** — creates contracts, ensures gates-policy requirements are covered by contracts/constraints
- **`/verify`** — validates everything is consistent, cross-checks gates vs contracts (catches gaps before /implement)
- **`/implement`** — executes plan tasks (both initial and remediation). Never decides what to build — only builds what the plan says
- **`/validate`** — runs gates, diagnoses failures. Never writes code — only creates FIX-tasks for /implement

### Workflow

```
/generate → /verify → /implement → /validate → release
                          ↑              │
                          │  if blocked: │
                          │  FIX-* tasks │
                          └──────────────┘
```

- **Happy path**: /generate → /verify → /implement → /validate → RELEASE APPROVED
- **Gate failure**: /validate adds FIX-* remediation tasks to the plan → /implement executes them → /validate re-runs
- **Human decision needed**: /validate adds open question → human answers → /implement → /validate
- **The human never runs technical commands.** The human writes artifacts, reviews contracts, and answers questions. Everything else is automated.

---

## 5. Agent Protocol

When executing a task from the plan:

1. Read `project.yaml` → find your task
2. Check `depends_on` → all dependencies completed
3. Load context: contract + glossary + stack + test spec + dependent code
4. Generate tests first (TDD): write tests with `@hlv` markers from contract error codes, invariants, and constraint rules
5. Generate implementation code to make the tests pass
6. Validate locally (compile, lint, unit tests, `hlv check` for marker coverage)
7. Update `project.yaml`: `task.status → completed`

Two agents NEVER write to the same file. Between groups — git commit.

---

## 6. Glossary

`human/glossary.yaml` is the canonical type dictionary. All contracts and code MUST use glossary types. No synonyms. No redefinitions.

---

## 7. Constraints

Global rules in `human/constraints/`:
- `security.yaml` — prepared statements, no secrets in logs, auth required
- `performance.yaml` — latency budgets, error rate limits, SLOs

These apply to ALL contracts unless explicitly excepted.
"#
    )
}

fn llm_map_template() -> String {
    r#"# yaml-language-server: $schema=../schema/llm-map-schema.json
# LLM Project Map — authoritative index of all project paths.
# hlv check validates every entry exists on disk.
# LLM agents MUST update this file when creating new files or directories.
schema_version: 1

# Patterns to exclude from reverse check (MAP-020).
# Add build artifacts, caches, and generated files for your stack.
ignore:
  - __pycache__
  - "*.pyc"
  - node_modules
  - target
  - dist
  - build
  - "*.egg-info"
  - .venv
  - .mypy_cache
  - .pytest_cache
  - .ruff_cache

entries:
  # --- Root ---
  - path: project.yaml
    kind: file
    layer: root
    description: "Project map — entry point for LLM agents, status, contracts, plan"

  # --- Human Layer ---
  - path: human/artifacts/
    kind: dir
    layer: human
    description: "Global project artifacts — domain context, tech stack, architectural decisions (shared across milestones)"
  - path: human/glossary.yaml
    kind: file
    layer: human
    description: "Domain types glossary — canonical types, enums, terms, naming rules"
  - path: human/constraints/
    kind: dir
    layer: human
    description: "Global constraints — security, performance, and observability rules for all contracts"
  - path: human/constraints/security.yaml
    kind: file
    layer: human
    description: "Security constraints — prepared statements, auth, PII masking, rate limits"
  - path: human/constraints/performance.yaml
    kind: file
    layer: human
    description: "Performance constraints — latency budgets, SLOs, error rate limits"
  - path: human/constraints/observability.yaml
    kind: file
    layer: human
    description: "Observability constraints — structured logging, request correlation, state change tracing"
  - path: human/milestones/
    kind: dir
    layer: human
    description: "Milestones — each milestone has artifacts, contracts, plan, stages"

  # --- Validation Layer ---
  - path: validation/scenarios/
    kind: dir
    layer: validation
    description: "Integration scenarios — cross-milestone end-to-end test flows"
  - path: validation/gates-policy.yaml
    kind: file
    layer: validation
    description: "Gate thresholds and release criteria (profile-dependent)"
  - path: validation/equivalence-policy.yaml
    kind: file
    layer: validation
    description: "Behavioral equivalence rules for controlled code regeneration"
  - path: validation/traceability-policy.yaml
    kind: file
    layer: validation
    description: "Traceability rules — ID formats, graph checks, reachability"
  - path: validation/ir-policy.yaml
    kind: file
    layer: validation
    description: "IR versioning — Contract IR and Test IR compatibility rules"
  - path: validation/adversarial-guardrails.yaml
    kind: file
    layer: validation
    description: "Adversarial LLM guardrails — redaction, read-only, provenance"
"#
    .to_string()
}

/// Interactive gate profile selection.
fn prompt_gate_profile() -> Result<GateProfile> {
    println!();
    println!(
        "  {} Gate profile — determines which validation gates are created:",
        "?".cyan().bold()
    );
    println!(
        "    {}  contract_tests + security (CLI, TUI, scripts, prototypes)",
        "1) minimal ".bold()
    );
    println!(
        "    {}  + integration_tests + property_based_tests (services, libraries)",
        "2) standard".bold()
    );
    println!(
        "    {}  + performance + mutation_testing + observability (production APIs)",
        "3) full    ".bold()
    );
    print!("  Choose [1/2/3, default=2]: ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin()
        .lock()
        .read_line(&mut line)
        .context("failed to read input")?;
    match line.trim() {
        "" | "2" | "standard" | "std" => Ok(GateProfile::Standard),
        "1" | "minimal" | "min" => Ok(GateProfile::Minimal),
        "3" | "full" => Ok(GateProfile::Full),
        other => anyhow::bail!(
            "Unknown profile: '{}'. Choose: 1 (minimal), 2 (standard), 3 (full)",
            other
        ),
    }
}

fn gates_policy_template(profile: GateProfile) -> String {
    let mut gates = String::new();

    // Core gates — always present
    gates.push_str(
        r#"  - id: GATE-CONTRACT-001
    type: contract_tests
    mandatory: true
    pass_criteria:
      required_scenarios_pass_rate: 1.0
  - id: GATE-SECURITY-001
    type: security
    mandatory: true
    pass_criteria:
      max_open_critical: 0
      max_open_high: 0
"#,
    );

    // Standard adds integration + PBT
    if matches!(profile, GateProfile::Standard | GateProfile::Full) {
        gates.push_str(
            r#"  - id: GATE-INTEGRATION-001
    type: integration_tests
    mandatory: true
    pass_criteria:
      p0_pass_rate: 1.0
      p1_min_pass_rate: 0.95
  - id: GATE-PBT-001
    type: property_based_tests
    mandatory: true
    pass_criteria:
      min_valid_generations_per_invariant: 10000
      counterexamples_allowed: 0
"#,
        );
    }

    // Full adds performance, mutation, observability
    if profile == GateProfile::Full {
        gates.push_str(
            r#"  - id: GATE-PERF-001
    type: performance
    mandatory: true
    pass_criteria:
      max_error_rate: 0.001
  - id: GATE-MUTATION-001
    type: mutation_testing
    mandatory: true
    pass_criteria:
      min_mutation_score_changed_modules: 0.70
  - id: GATE-OBS-001
    type: observability
    mandatory: true
    pass_criteria:
      required_for_public_capabilities: [metrics, trace_spans, structured_logs]
"#,
        );
    }

    format!(
        r#"# yaml-language-server: $schema=../schema/gates-policy-schema.json
# Gate profile: {profile}
# /generate may adjust gates based on project analysis.
# /implement sets `command` for each gate after code is generated.
# Manage gates: hlv gates enable/disable/set-cmd/clear-cmd <GATE-ID>
version: 1.0.0
policy_id: HLV-VAL-GATES
description: "Validation gates and release thresholds (profile: {profile})."

release_policy:
  require_all_mandatory: true
  flaky_policy:
    quarantine_required: true
    block_release_for_p0: true

gates:
{gates}"#,
        profile = profile.label(),
        gates = gates,
    )
}

static EQUIV_POLICY_TEMPLATE: &str = r#"# yaml-language-server: $schema=../schema/equivalence-policy-schema.json
version: 1.0.0
policy_id: HLV-VAL-EQUIV
description: Behavioral equivalence policy for controlled code regeneration.

scope:
  applies_to: unchanged_contracts
  required_for_regeneration: true

requirements:
  - id: EQUIV-001
    rule: fixed_test_ir
    must: true
    details: "Comparison must use a fixed Test IR version and unchanged seed set."
  - id: EQUIV-002
    rule: nondeterminism_normalization
    must: true
  - id: EQUIV-003
    rule: deterministic_external_io
    must: true
  - id: EQUIV-004
    rule: comparison_dimensions
    must: true
  - id: EQUIV-005
    rule: explicit_tolerances
    must: true
"#;

static TRACE_POLICY_TEMPLATE: &str = r#"# yaml-language-server: $schema=../schema/traceability-policy-schema.json
version: 1.0.0
policy_id: HLV-VAL-TRACE
description: Machine-verifiable traceability policy.

id_formats:
  requirement: "^REQ-[a-z0-9-]+-[0-9]{4}$"
  contract: "^CTR-[a-z0-9.-]+-[0-9]+\\.[0-9]+\\.[0-9]+$"
  test: "^TST-[a-z0-9-]+-[0-9]{4}$"
  gate: "^GATE-[A-Z0-9-]+-[0-9]{3}$"

graph_requirements:
  required_paths:
    - "requirement -> contract"
    - "contract -> test"
    - "test -> gate"
  checks:
    - id: TRACE-001
      name: no_dangling_references
      must: true
    - id: TRACE-002
      name: requirement_reachability
      must: true
      rule: "each requirement must reach at least one contract and one test"
    - id: TRACE-003
      name: test_gate_mapping
      must: true
      rule: "each test must map to at least one runtime or ci gate"
"#;

static IR_POLICY_TEMPLATE: &str = r#"# yaml-language-server: $schema=../schema/ir-policy-schema.json
version: 1.0.0
policy_id: HLV-VAL-IR
description: Compatibility and reproducibility policy for Contract IR and Test IR.

compatibility_rules:
  - id: IR-001
    must: true
    rule: "Every Contract IR and Test IR document must include ir_schema_version."

required_fields:
  contract_ir: [ir_schema_version, compiler_version, source_hash, contract_id, version]
  test_ir: [ir_schema_version, source_hash, contract_id, contract_version, test_cases, gate_mappings]
"#;

static ADV_GUARDRAILS_TEMPLATE: &str = r#"# yaml-language-server: $schema=../schema/adversarial-guardrails-schema.json
version: 1.0.0
policy_id: HLV-VAL-ADV
description: Mandatory guardrails for using an Adversarial LLM in validation.

requirements:
  - id: ADV-001
    must: true
    rule: "Input artifacts must pass secrets and PII redaction before model access."
  - id: ADV-002
    must: true
    rule: "Adversarial model execution must run in read-only mode."
  - id: ADV-003
    must: true
    rule: "Every finding must include provenance: model, prompt_hash, artifact_hash."
  - id: ADV-004
    must: true
    rule: "No finding is accepted without a reproducible test or enforceable policy rule."
"#;

#[cfg(test)]
mod tests {
    use super::*;

    // --- GateProfile ---

    #[test]
    fn gate_profile_from_str_valid() {
        assert_eq!(
            GateProfile::from_str_opt("minimal").unwrap(),
            GateProfile::Minimal
        );
        assert_eq!(
            GateProfile::from_str_opt("min").unwrap(),
            GateProfile::Minimal
        );
        assert_eq!(
            GateProfile::from_str_opt("standard").unwrap(),
            GateProfile::Standard
        );
        assert_eq!(
            GateProfile::from_str_opt("std").unwrap(),
            GateProfile::Standard
        );
        assert_eq!(
            GateProfile::from_str_opt("full").unwrap(),
            GateProfile::Full
        );
        assert_eq!(
            GateProfile::from_str_opt("FULL").unwrap(),
            GateProfile::Full
        );
    }

    #[test]
    fn gate_profile_from_str_invalid() {
        let result = GateProfile::from_str_opt("extreme");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown gate profile"));
    }

    #[test]
    fn gate_profile_label() {
        assert_eq!(GateProfile::Minimal.label(), "minimal");
        assert_eq!(GateProfile::Standard.label(), "standard");
        assert_eq!(GateProfile::Full.label(), "full");
    }

    // --- detect_agent ---

    #[test]
    fn detect_agent_found() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".claude/skills")).unwrap();

        let agent = detect_agent(root).unwrap();
        assert_eq!(agent, "claude");
    }

    #[test]
    fn detect_agent_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let result = detect_agent(dir.path());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot detect agent"));
    }

    #[test]
    fn detect_agent_ignores_dot_only() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // "./" directory shouldn't match
        fs::create_dir_all(root.join("./skills")).unwrap();
        // Regular hidden dir without skills subdir
        fs::create_dir_all(root.join(".git")).unwrap();

        let result = detect_agent(root);
        assert!(result.is_err());
    }

    // --- write_template ---

    #[test]
    fn write_template_creates_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        write_template(root, "test.txt", "hello").unwrap();
        assert_eq!(fs::read_to_string(root.join("test.txt")).unwrap(), "hello");
    }

    #[test]
    fn write_template_skips_existing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("test.txt"), "original").unwrap();
        write_template(root, "test.txt", "overwrite").unwrap();
        assert_eq!(
            fs::read_to_string(root.join("test.txt")).unwrap(),
            "original",
            "should not overwrite"
        );
    }

    #[test]
    fn write_template_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        write_template(root, "a/b/c/deep.txt", "nested").unwrap();
        assert_eq!(
            fs::read_to_string(root.join("a/b/c/deep.txt")).unwrap(),
            "nested"
        );
    }

    // --- write_or_update ---

    #[test]
    fn write_or_update_creates_new() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        write_or_update(root, "new.txt", "content").unwrap();
        assert_eq!(fs::read_to_string(root.join("new.txt")).unwrap(), "content");
    }

    #[test]
    fn write_or_update_skips_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("same.txt"), "content").unwrap();
        let mtime_before = fs::metadata(root.join("same.txt"))
            .unwrap()
            .modified()
            .unwrap();

        write_or_update(root, "same.txt", "content").unwrap();
        let mtime_after = fs::metadata(root.join("same.txt"))
            .unwrap()
            .modified()
            .unwrap();

        assert_eq!(mtime_before, mtime_after, "should not touch unchanged file");
    }

    #[test]
    fn write_or_update_overwrites_changed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("old.txt"), "version 1").unwrap();
        write_or_update(root, "old.txt", "version 2").unwrap();
        assert_eq!(
            fs::read_to_string(root.join("old.txt")).unwrap(),
            "version 2"
        );
    }

    // --- gates_policy_template ---

    #[test]
    fn gates_policy_template_minimal() {
        let t = gates_policy_template(GateProfile::Minimal);
        assert!(t.contains("GATE-CONTRACT-001"));
        assert!(t.contains("GATE-SECURITY-001"));
        assert!(!t.contains("GATE-INTEGRATION-001"));
        assert!(!t.contains("GATE-PERF-001"));
    }

    #[test]
    fn gates_policy_template_standard() {
        let t = gates_policy_template(GateProfile::Standard);
        assert!(t.contains("GATE-CONTRACT-001"));
        assert!(t.contains("GATE-INTEGRATION-001"));
        assert!(t.contains("GATE-PBT-001"));
        assert!(!t.contains("GATE-PERF-001"));
    }

    #[test]
    fn gates_policy_template_full() {
        let t = gates_policy_template(GateProfile::Full);
        assert!(t.contains("GATE-PERF-001"));
        assert!(t.contains("GATE-MUTATION-001"));
        assert!(t.contains("GATE-OBS-001"));
    }

    // --- ensure_project_yaml_schema ---

    #[test]
    fn ensure_project_yaml_schema_adds_missing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(
            root.join("project.yaml"),
            "schema_version: 1\nproject: test\n",
        )
        .unwrap();

        ensure_project_yaml_schema(root).unwrap();

        let content = fs::read_to_string(root.join("project.yaml")).unwrap();
        assert!(content.contains("yaml-language-server: $schema="));
        assert!(content.contains("spec: schema/project-schema.json"));
    }

    #[test]
    fn ensure_project_yaml_schema_skips_existing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let original = "# yaml-language-server: $schema=schema/project-schema.json\nschema_version: 1\nspec: schema/project-schema.json\nproject: test\n";
        fs::write(root.join("project.yaml"), original).unwrap();

        ensure_project_yaml_schema(root).unwrap();

        let content = fs::read_to_string(root.join("project.yaml")).unwrap();
        assert_eq!(content, original, "should not modify");
    }

    // --- ensure_yaml_schemas ---

    #[test]
    fn ensure_yaml_schemas_adds_to_glossary() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("human")).unwrap();
        fs::write(root.join("human/glossary.yaml"), "domain: test\n").unwrap();

        ensure_yaml_schemas(root).unwrap();

        let content = fs::read_to_string(root.join("human/glossary.yaml")).unwrap();
        assert!(content.contains("yaml-language-server: $schema="));
    }
}
