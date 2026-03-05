# AGENTS.md — Meta-rules for LLM agents working on HLV itself

This file defines invariants that any agent modifying the HLV codebase MUST follow.

---

## Core Invariant

**Every schema change must ripple through all layers.**

When you change the data model (`project.yaml` schema, model structs, diagnostic codes), you MUST update every dependent layer in the same changeset:

1. **Model** — `src/model/project.rs` (structs, enums, serde attributes)
2. **Validation** — `src/check/*.rs` (diagnostic rules)
3. **Tests** — `tests/model_parse.rs` (parse tests), `tests/check_tests.rs` (check tests)
4. **Skills** — `skills/*/SKILL.md` (generation, verification, implementation, validation)
5. **Documentation** — `docs/ARCH.md`, `docs/WORKFLOW.md`

No partial changes. If you add a field to `project.yaml`, all five layers must reflect it.

---

## Checklists

### Adding a new section to `project.yaml`

- [ ] Add struct(s) in `src/model/project.rs` with `Deserialize, Serialize, Clone`
- [ ] Add `Option<NewSection>` with `#[serde(default)]` to `ProjectMap`
- [ ] Create `src/check/<section>.rs` with validation function returning `Vec<Diagnostic>`
- [ ] Register module in `src/check/mod.rs`
- [ ] Wire into `src/check/project_map.rs` (call when `Some`)
- [ ] Add parse test in `tests/model_parse.rs` (load real `project.yaml`, assert structure)
- [ ] Add check tests in `tests/check_tests.rs` (helpers + error/warning/valid cases)
- [ ] Update `skills/generate/SKILL.md` — extraction step + project.yaml output
- [ ] Update `skills/implement/SKILL.md` — read step + agent context
- [ ] Update `skills/verify/SKILL.md` — add structural checks for new section
- [ ] Update `skills/validate/SKILL.md` — note availability for gate context
- [ ] Update `docs/ARCH.md` section 5.1 — add to project.yaml description
- [ ] Update `docs/WORKFLOW.md` Phase 2 table — add row if user-visible
- [ ] `cargo test` — all pass
- [ ] `cargo clippy` — no warnings

### Adding a new diagnostic code

- [ ] Choose prefix from registry below
- [ ] Implement in `src/check/<module>.rs`
- [ ] Add test in `tests/check_tests.rs`
- [ ] `cargo test` — all pass

### Changing a skill

- [ ] Update `skills/<name>/SKILL.md`
- [ ] If the skill reads new data — ensure model structs exist
- [ ] If the skill writes new data — ensure validators cover it
- [ ] Verify `hlv check` still passes on example project

### Adding a milestone command

- [ ] Add function in `src/cmd/milestone.rs`
- [ ] Wire into `MilestoneAction` enum in `src/main.rs`
- [ ] Add integration test in `tests/milestone_tests.rs`
- [ ] `cargo test` — all pass
- [ ] `cargo clippy` — no warnings

### Modifying model structs

- [ ] Update `src/model/project.rs`
- [ ] Update or add serde tests in `tests/model_parse.rs`
- [ ] Ensure `project.yaml` fixture matches new schema
- [ ] Check all validators still compile and pass

---

## Diagnostic Code Prefix Registry

| Prefix | Module | Description |
|--------|--------|-------------|
| PRJ | project_map | Project map (project.yaml) |
| CTR | contracts, code_trace | Contract markdown + YAML; `@hlv` code traceability markers |
| TST | validation | Test specifications |
| TRC | traceability | Traceability mappings |
| PLN | plan | Implementation plan |
| STK | stack | Tech stack components |
| MST | milestone | Milestone tracking (milestones.yaml) |

When adding a new validation domain, register a new prefix here.

Note: `/artifacts` is a skill (not a validation domain) — it writes to `human/artifacts/` via interactive interview. No diagnostic codes.

---

## Testing Protocol

1. `cargo test` — all existing + new tests pass
2. `cargo clippy` — no warnings
3. `hlv check --root tests/fixtures/example-project` — new diagnostics appear correctly

---

## Post-Task Rule

**After every task, run `make lint` and fix any issues before considering the task complete.**

`make lint` runs `cargo clippy -- -D warnings` and `cargo fmt -- --check`. Both must pass.
