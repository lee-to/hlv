# HLV Project Structure

Canonical structure created by `hlv init` + `hlv milestone new`.
Source of truth for fixture validation and legacy detection.

```
project.yaml                        # entry point - status, paths, stack, constraints, features
milestones.yaml                     # current milestone, stages, history
HLV.md                              # methodology rules (auto-generated)
AGENTS.md                           # project-specific rules (user-editable)

human/
  artifacts/                         # global context (flat files)
    context.md                       #   domain, users, business context
    stack.md                         #   technical stack
    constraints.md                   #   hard constraints
    <decision>.md                    #   architectural decisions (ADR)
  glossary.yaml                      # domain types (shared across milestones)
  constraints/                       # global constraints (YAML)
    security.yaml
    performance.yaml
    observability.yaml
  traceability.yaml                  # REQ -> Contract -> Test -> Gate map
  milestones/
    <NNN-slug>/                      # milestone directory (created by hlv milestone new)
      artifacts/                     #   milestone-specific artifacts (flat files)
        <feature>.md                 #     feature description
        unknowns.md                  #     open questions and risks
        <decision>.md                #     milestone-specific decisions
      contracts/                     #   contracts (created by /hlv-generate)
        <contract-id>.md             #     markdown specification
        <contract-id>.yaml           #     machine-readable IR
      test-specs/                    #   test specifications (created by /hlv-generate)
        <contract-id>.md
      plan.md                        #   scope, stage table
      stage_N.md                     #   tasks for each stage
      traceability.yaml              #   per-milestone traceability
      open-questions.md              #   unresolved questions

validation/
  gates-policy.yaml                  # gates and thresholds
  equivalence-policy.yaml            # behavioral equivalence rules
  traceability-policy.yaml           # traceability rules
  ir-policy.yaml                     # IR versioning
  adversarial-guardrails.yaml        # adversarial LLM protection
  test-specs/                        # (empty, created by init)
  scenarios/                         # integration scenarios
    <scenario>.md

llm/
  src/                               # generated code
  tests/                             # generated tests
  map.yaml                           # index of all project files

schema/                              # JSON schemas (copied during init)
  project-schema.json
  milestones-schema.json
  glossary-schema.json
  contract-schema.json
  security-constraints-schema.json
  performance-constraints-schema.json
  traceability-schema.json
  llm-map-schema.json
  signatures-schema.json
  gates-policy-schema.json
  equivalence-policy-schema.json
  traceability-policy-schema.json
  ir-policy-schema.json
  adversarial-guardrails-schema.json

.<agent>/                            # agent directory (for example .claude/)
  skills/                            # skills deployed during init
    artifacts/SKILL.md
    generate/SKILL.md
    verify/SKILL.md
    implement/SKILL.md
    validate/SKILL.md
    questions/SKILL.md
```

## Adopted Project Layout

`hlv init --adopt` keeps repository code in place and writes HLV-owned artifacts under `.hlv/`:

```
AGENTS.md                            # root-owned project rules
HLV.md                               # root-owned methodology rules
.gitignore                           # includes .hlv/index/ by default

.hlv/project.yaml                    # entry point; sets hlv_root: .hlv
.hlv/milestones.yaml
.hlv/human/
.hlv/validation/
.hlv/llm/
  map.yaml                           # includes layer: code entries for observed roots
.hlv/index/
  signatures.yaml                    # generated signature index, usually ignored

app/, src/, cmd/, internal/, tests/   # observed legacy roots from project.yaml -> paths.code
```

In adopted projects, `paths.code` points at observed legacy source/test roots relative to the repository root. Existing code is not moved into `.hlv/llm/src/`; the full contract flow applies to new or changed milestone work.

## Lifecycle

```
status: draft -> verified -> implementing -> implemented -> validating -> validated
```

Status is per-stage in `milestones.yaml`, not global.

## What Is NOT Part of the Structure

The following concepts were removed and are considered legacy:

| Concept | Reason for removal |
|-----------|-----------------|
| `human/contracts/` (global) | Contracts are now per-milestone |
| `human/plan.md` (global) | Plan is now per-milestone |
| `human/scenarios/` | Scenarios live in `validation/scenarios/` |
| `human/changes/` | Contract change log was not implemented |
| `human/artifacts/tasks/`, `decisions/`, `infra/`, `research/` | Artifacts are flat files without subdirectories |
| `contracts: []` in `project.yaml` | Contracts are scanned from the milestone directory |
| `open_questions: []` in `project.yaml` | Open questions are in per-milestone `open-questions.md` |
| `status: contracts_generated` / `contracts_verified` | Removed - lifecycle simplified |
| `schema/scenario-schema.json` | No model, not validated |
| `schema/change-schema.json` | No model, not validated |
