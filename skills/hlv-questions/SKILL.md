---
name: hlv-questions
description: Interactive session to resolve open questions. Walks through each question, gives a recommendation, asks the user, and updates the file. Use after /hlv-generate when open questions exist.
disable-model-invocation: true
allowed-tools: Read Write Edit Glob Grep
metadata:
  author: hlv
  version: "1.0"
---

# HLV Questions — Interactive Open Questions Resolution

You are helping the user resolve open questions that block `/hlv-verify`. You walk through each question one by one, give a recommendation based on project context, ask the user to decide, and update the files.

## Prerequisites

- `project.yaml` exists
- `milestones.yaml` exists with a `current` section
- Open questions file exists with unresolved questions (`- [ ]`)
- Read `project.yaml` to get paths and context
- Read all artifacts referenced in the questions' `source:` fields

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

## Locate Files

1. Read `milestones.yaml` → get `current.id` (referred to as `{MID}` below)
2. Open questions file: `human/milestones/{MID}/open-questions.md`
3. Contracts context: `{MID}/contracts/`
4. Artifacts context: `human/milestones/{MID}/artifacts/`

## Core Rules

1. **Write in the user's language.** Match the language of `open-questions.md`.
2. **One question at a time.** Never dump all questions. Present one, wait for the answer, then move to the next.
3. **Always recommend.** For every question, give a concrete recommendation based on the project artifacts, contracts, and common engineering practice. Explain your reasoning in 1-2 sentences.
4. **Three actions.** For each question the user can:
   - **Answer** — provide a decision → you mark `[x]` and record the answer
   - **Defer** — not critical now → you mark `[deferred]` with a reason
   - **Skip** — come back later → leave `[ ]`, move to next
5. **Never invent requirements.** Your recommendation is a suggestion. The user decides. If they disagree with your recommendation, use their answer.
6. **Update files immediately.** After each answer, update `open-questions.md` right away. Don't batch.
7. **Update contracts if needed.** When an answer changes contract behavior (new field, new error case, changed invariant), note it and tell the user which contracts need updating. Do NOT update contracts in this skill — that's `/hlv-generate`'s job.

## Flow

### Step 1: Load context

1. Read `project.yaml`
2. Read `milestones.yaml` → get `current.id` (`{MID}`)
3. Read `human/milestones/{MID}/open-questions.md`
4. Count open questions (`- [ ]`)
5. Read the source artifacts referenced by each question
6. Read relevant contracts from `{MID}/contracts/` to understand what each question affects

### Step 2: Present summary

```
Open questions: <N> total (<M> resolved, <K> deferred)

Categories:
  <category 1>: <count> questions
  <category 2>: <count> questions

Let's go through them. I'll recommend an answer for each one.
```

### Step 3: Walk through questions

For each unresolved `- [ ]` question, present:

```
── Question <N>/<total> ──────────────────────────

  <question text>

  Source: <artifact file>
  Affects: <which contracts or components this impacts>

  Recommendation: <your concrete suggestion>
  Reasoning: <why — based on artifacts, constraints, common practice>

  → answer / defer / skip?
```

**How to recommend:**

- Read the source artifact for context
- Check if contracts already imply an answer
- Consider project constraints (stack, NFR, infrastructure)
- For UX questions — suggest the simpler option for MVP, note it can be changed later
- For technical questions — suggest the standard/boring solution unless constraints require otherwise
- For infrastructure questions — suggest what matches the existing stack

**After user responds:**

- **Answer given** → update the line in `open-questions.md`:
  ```markdown
  - [x] <question text> — source: <artifact>
    → <user's answer>
  ```

- **Deferred** → update the line:
  ```markdown
  - [deferred] <question text> — source: <artifact>
    → deferred: <reason>
  ```

- **Skipped** → leave as is, move to next

### Step 4: Update project files

The open-questions.md file is already updated in Step 3 (each answer is written immediately). No `project.yaml` update needed — open questions live in the milestone directory, not in project.yaml.

### Step 5: Summary

```
── Questions resolved ────────────────────────────

  Answered:  <N>
  Deferred:  <K>
  Skipped:   <M>
  Remaining: <R> open (blockers for /hlv-verify)

  Contracts affected by answers:
    - <contract.id>: <what changed — e.g. "new phone format constraint">
    - <contract.id>: <what changed>
```

If remaining == 0:
```
All questions resolved. Run /hlv-generate to update contracts with the new answers, then /hlv-verify.
```

If remaining > 0:
```
<R> questions still open — these block /hlv-verify. Run /hlv-questions again when ready.
```

## Tips

- **Group related questions.** If multiple questions are about the same topic, mention that context carries over.
- **Speed is good.** Most questions have obvious answers. Don't over-explain — give a quick recommendation and let the user confirm or override.
- **Defer is fine.** Not everything needs an answer now. Infrastructure details can wait until `/hlv-implement`. UX copy can wait until after MVP. Help the user understand what's truly blocking and what can be deferred.
- **Watch for cascading answers.** One answer may resolve or change another question. If you notice this, tell the user: "Your answer to Q3 also answers Q7 — should I mark it resolved too?"

## Cleanup

After the skill completes:
1. Run `hlv check` to validate the project structure.
2. If answers affect contracts, suggest running `/hlv-generate` to update them.
3. Suggest `/clear` to free context before the next skill.
