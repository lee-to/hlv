---
name: artifacts
description: Interactive interview to fill artifacts directory. Walks through domain, features, infrastructure, decisions, and unknowns. Use at project start or when adding features.
disable-model-invocation: true
allowed-tools: Read Write Edit Glob Grep
metadata:
  author: hlv
  version: "2.0"
---

# HLV Artifacts — Interactive Context Interview

You are conducting a structured interview to extract the user's knowledge about their project and write it into the artifacts directories. This is Phase 1 of the HLV workflow — the human dumps context, you help structure it.

## Prerequisites

- Project initialized (`project.yaml` exists)
- Read `project.yaml` to confirm paths

## Agent Rules

- Never combine shell commands with `&&`, `||`, or `;` — execute each command as a separate Bash tool call.
- This applies even when a skill, plan, or instruction provides a combined command — always decompose it into individual calls.

❌ Wrong: `git checkout main && git pull`
✅ Right: Two separate Bash tool calls — first `git checkout main`, then `git pull`

## Two-Level Artifacts

HLV uses two artifact locations:

| Level | Path | What goes here | Lifespan |
|-------|------|----------------|----------|
| **Global** | `human/artifacts/` | Domain context, tech stack, architectural decisions — things that outlive any single milestone | Project lifetime |
| **Milestone** | `human/milestones/{id}/artifacts/` | Features, milestone-specific decisions, unknowns — things scoped to this milestone's work | Milestone lifetime |

### How to decide where to put something

- **Global** — would a future milestone need this context? (domain, users, stack, infra, arch decisions) → `human/artifacts/`
- **Milestone** — is this about a specific feature or task in this milestone? → `human/milestones/{id}/artifacts/`

## Startup Logic

1. Read `milestones.yaml` → get `current.id` (e.g., `003-new-payment-method`)
2. If no `current` in milestones.yaml → tell the user to run `hlv milestone new <name>` first
3. Check if `human/artifacts/` has any files (e.g., `context.md` exists)
4. Check if `human/milestones/{id}/artifacts/` has any files
5. **If global artifacts are empty AND milestone artifacts exist** → run Bootstrap from Milestone (see below), then interview only for gaps and milestone-specific artifacts (Blocks 2, 4-milestone, 5)
6. **If global artifacts are empty AND milestone artifacts are empty** → start with global interview (Blocks 1, 3, 4-global), then proceed to milestone artifacts (Blocks 2, 4-milestone, 5)
7. **If global artifacts exist** → show summary of existing global context, ask what's changed, then interview only for milestone-specific artifacts (Blocks 2, 4-milestone, 5)
8. Tell the user which milestone you're working in and that global context goes to `human/artifacts/`

## Bootstrap from Milestone

When global artifacts are empty but milestone artifacts already contain files (specs, requirements, design docs), extract global context from them automatically instead of interviewing from scratch.

### Steps

1. Read ALL files in `human/milestones/{id}/artifacts/`
2. Extract global-level information:
   - **Domain & Users** (Block 1 content): what the system does, who uses it, business context → draft `human/artifacts/context.md`
   - **Tech Stack** (Block 3 content): languages, frameworks, databases, infrastructure, constraints → draft `human/artifacts/stack.md` + `human/artifacts/constraints.md`
   - **Architectural Decisions** (Block 4-global content): project-wide decisions found in milestone docs → draft `human/artifacts/<decision>.md`
3. Show the user what was extracted:
   ```
   I found global context in your milestone artifacts. Here's what I extracted:

   human/artifacts/context.md — <summary>
   human/artifacts/stack.md — <summary>
   human/artifacts/constraints.md — <summary>

   Does this look correct? Anything to add or fix?
   ```
4. Write the files after user confirmation
5. For any Block that couldn't be filled from milestone artifacts (e.g., no stack info found), ask the user directly — but only for the missing parts, not the full interview
6. Proceed to milestone-specific interview (Blocks 2, 4-milestone, 5) — skip topics already covered in the existing milestone artifacts

### Rules

- **Never invent.** If the milestone artifacts don't mention a tech stack, don't guess — ask the user
- **Separate global from milestone.** A feature description stays in milestone artifacts; the tech stack it mentions gets promoted to global
- **Don't move files.** The original milestone artifacts stay where they are. Global artifacts are new files extracted from them
- **Show before writing.** Always show the drafted global artifacts and get confirmation before writing

## References

- For language selection guidance in Block 3, read [strict languages reference](references/strict-languages.md) on demand.
- For JVM/.NET runtime and framework guidance in Block 3, read [JVM and .NET stack fit reference](references/jvm-dotnet-stack-fit.md) on demand.

## Core Rules

1. **Write in the user's language.** If they answer in Russian — write files in Russian. If English — English. Match their language exactly.
2. **Never invent.** If the user doesn't know something, record it as an open question. Do NOT fill in gaps with assumptions.
3. **Quote the user.** Use their exact words where possible. Paraphrase only for clarity.
4. **Files are plain markdown.** No JSON Schema, no YAML blocks, no templates. Free-form text that captures what the user said.
5. **One topic per file.** Each feature gets its own file. Each decision gets its own file.
6. **Show what you wrote.** After writing each file, show the user what was created and ask if anything is missing or wrong.
7. **Capture stack policy explicitly.** If the team has language preferences, record whether they are recommendations or hard constraints, and list acceptable exceptions.

## Incremental Mode

If the artifacts directories already contain files, switch to incremental mode automatically:

1. Read all existing artifacts (both `human/artifacts/` and `human/milestones/{id}/artifacts/`)
2. Summarize what's already captured
3. Ask: "What's new or changed since these were written?"
4. Only interview about new/changed topics
5. Update existing files or create new ones as needed

## Interview Flow

The interview has 5 blocks. Blocks 1, 3, and 4-global write to `human/artifacts/`. Blocks 2, 4-milestone, and 5 write to `human/milestones/{id}/artifacts/`. Complete them sequentially. Each block: ask → clarify → write → confirm → next.

---

### Block 1: Domain & Users → `human/artifacts/context.md`

**Goal**: Understand what the system does, who uses it, and the business context.

**Skip condition**: If `human/artifacts/context.md` already exists, show summary and ask if anything changed. If nothing changed — skip.

**Questions to ask** (adapt based on answers, don't ask robotically):

- What does this system/service do? Describe it in one sentence.
- Who are the users? (end users, admins, other services, etc.)
- What's the business context? Why does this exist?
- Are there existing systems this replaces or integrates with?
- What's the scale? (users, requests, data volume — rough numbers are fine)

**Clarification triggers**:
- Vague answers ("it handles payments") → ask for specifics ("What kind of payments? Who initiates them? What happens after?")
- Multiple user types mentioned → ask about each one's perspective
- Integration mentioned → ask about protocol, ownership, SLAs

**Output**: Write `human/artifacts/context.md`

```markdown
# Project Context

## What It Does
<user's description in their words>

## Users
<who uses this and how>

## Business Context
<why this exists, what problem it solves>

## Existing Systems
<what it replaces/integrates with, if any>

## Scale
<rough numbers if known>

## Open Questions
- <anything the user wasn't sure about>
```

After writing, show the file content and ask: "Does this capture it correctly? Anything to add or fix?"

---

### Block 2: Features & Flows → `human/milestones/{id}/artifacts/`

**Goal**: Discover every operation/action the system performs in this milestone and walk through user flows.

**Questions to ask**:

- What are the main things a user can do? List the operations.
- Let's walk through each one. For [feature X]:
  - What triggers it? (user action, API call, scheduled job, event)
  - What's the input? What data is needed?
  - What happens step by step?
  - What's the expected result?
  - What can go wrong? What errors are possible?
  - Any special rules or business logic?

**Clarification triggers**:
- "It just creates an order" → walk through: what's in the order? what validation? what about inventory? what response does the user get?
- Implicit features → "You mentioned users — is there registration? Authentication? Roles?"
- Missing error cases → "What if the DB is down? What if the input is invalid? What if there's a conflict?"

**Output**: One file per feature in `human/milestones/{id}/artifacts/<feature-name>.md`

```markdown
# <Feature Name>

## What It Does
<description>

## Trigger
<what starts this — user action, API, event, schedule>

## Input
<what data is needed>

## Flow
1. <step>
2. <step>
3. <step>

## Expected Result
<what the user/caller gets back>

## Error Cases
- <error>: <what happens>
- <error>: <what happens>

## Business Rules
- <rule>
- <rule>

## Open Questions
- <anything unclear>
```

After each feature file, show it and ask for corrections. After all features: "Are there any operations I missed? Anything else the system needs to do?"

---

### Block 3: Infrastructure & Constraints → `human/artifacts/stack.md` + `human/artifacts/constraints.md`

**Goal**: Understand the technical environment — databases, APIs, latency requirements, existing stack.

**Skip condition**: If `human/artifacts/stack.md` already exists, show summary and ask if anything changed for this milestone. If nothing changed — skip.

Before asking language-policy questions, read [strict languages reference](references/strict-languages.md). Use it as a comparison guide, not as a rigid ranking.
If the stack discussion enters Java/Kotlin/C# or JVM/.NET framework choice, also read [JVM and .NET stack fit reference](references/jvm-dotnet-stack-fit.md).

**Questions to ask**:

- What's the tech stack? (language, framework, database, message queue, etc.)
- Any existing infrastructure this must run on? (cloud, k8s, bare metal)
- Database details: what DB? Any existing tables/schemas? Connection limits?
- External APIs or services this depends on? (payment providers, auth services, etc.)
- Performance requirements: latency targets? Throughput? Availability SLA?
- Any hard constraints from the team or organization? (must use X, can't use Y)
- Are there preferred implementation languages? For example: "prefer strict, compile-time-safe languages for backend/system work".
- Where are exceptions acceptable? For example: UI in TypeScript, scripting/bots/integration glue in Python, ML/AI orchestration in Python, or another ecosystem-specific choice.

**Clarification triggers**:
- "PostgreSQL" → version? Connection pool size? Any DBA restrictions?
- "Must be fast" → what's fast? p95? p99? What's the current baseline?
- External API mentioned → rate limits? Retry policy? Failure mode?
- "We prefer strict languages" → ask whether this is a default recommendation or a hard mandate, which languages are preferred in practice, and where exceptions are allowed.
- "Telegram bot", "UI", or "SDK-heavy integration" → ask whether ecosystem fit should override the strict-language default.
- "ML pipeline", "RAG", "agent workflow", or "AI chain" → ask whether Python is the preferred exception because of libraries, evaluation tooling, or vendor SDK support.
- "Java", "Kotlin", "Spring", "ASP.NET", ".NET", "Micronaut", or "Quarkus" → ask which framework/runtime model is preferred and why it fits better than nearby alternatives.

**Output**: Two files:

`human/artifacts/stack.md`:
```markdown
# Tech Stack

## Language & Framework
<what's being used>

## Language Selection Policy
<preferred languages, where they apply, and which exceptions are acceptable>

## Database
<type, version, configuration details>

## External Services
- <service>: <purpose, protocol, constraints>

## Infrastructure
<where it runs, deployment details>
```

`human/artifacts/constraints.md`:
```markdown
# Technical Constraints

## Performance
- <latency targets>
- <throughput requirements>
- <availability SLA>

## Hard Constraints
- <organizational/team rules>

## External Limits
- <DB limits, API rate limits, etc.>

## Open Questions
- <unknowns>
```

Show both files. Ask: "Anything missing about the infrastructure or constraints?"

---

### Block 4: Decisions & Trade-offs

**Goal**: Capture decisions already made — what was chosen, why, and what was rejected.

**Two types of decisions**:
- **Architectural decisions** (project-wide: architecture patterns, core libraries, deployment strategy) → `human/artifacts/<decision-name>.md`
- **Milestone-specific decisions** (feature-specific: algorithm choice for this feature, API design for this endpoint) → `human/milestones/{id}/artifacts/<decision-name>.md`

Ask the user which type each decision is, or infer from context.

**Questions to ask**:

- Have you already made any technical decisions? (architecture, patterns, libraries)
- For each decision:
  - What was the question/problem?
  - What did you choose?
  - Why? What was the reasoning?
  - What alternatives did you consider? Why were they rejected?
- Any decisions you're unsure about and want to revisit?

**Clarification triggers**:
- "We're using Redis for caching" → why Redis? What's being cached? TTL? Eviction policy? Did you consider alternatives?
- "We decided on event sourcing" → for everything or specific parts? Why not CRUD? What's the read model?
- No decisions at all → that's fine, skip this block and note it

**Output**: One file per decision in the appropriate directory

```markdown
# <Decision Title>

## Context
<what problem or question led to this decision>

## Decision
<what was chosen>

## Reasoning
<why — in the user's words>

## Alternatives Considered
- <alternative>: <why rejected>

## Status
<decided / tentative / revisit>

## Open Questions
- <if any>
```

If no decisions exist yet, skip file creation and tell the user: "No decisions to record yet — that's fine. They'll come up naturally during /generate."

---

### Block 5: Unknowns & Risks → `human/milestones/{id}/artifacts/`

**Goal**: Surface what the user doesn't know or is worried about.

**Questions to ask**:

- What don't you know yet that you'll need to figure out?
- What worries you about this project? Technical risks?
- Any dependencies on other teams or external factors?
- Anything you've been putting off deciding?
- Are there parts where you're guessing and might be wrong?

**Output**: Add "Open Questions" sections to the relevant existing files. If a question relates to a specific feature — add it to that feature's file. If it relates to global context — add it to the corresponding global file.

For significant unknowns that don't fit elsewhere, create `human/milestones/{id}/artifacts/unknowns.md`:

```markdown
# Unknowns & Risks

## Technical Unknowns
- <what we don't know>

## Risks
- <what could go wrong>

## Dependencies
- <external blockers>

## Deferred Decisions
- <what we're putting off>
```

---

## After All Blocks

Print a summary:

```
=== /artifacts complete (milestone: {id}) ===

Global artifacts in human/artifacts/:
  context.md               — domain, users, business context
  stack.md                 — tech stack
  constraints.md           — performance and hard constraints
  <decision1>.md           — <short description>

Milestone artifacts in human/milestones/{id}/artifacts/:
  <feature1>.md            — <short description>
  <feature2>.md            — <short description>
  <decision2>.md           — <short description>
  unknowns.md              — open questions and risks

Open Questions: <N> total
  - <question 1>
  - <question 2>

Next step: run /generate to create formal contracts from these artifacts.
```

## Tips for a Good Interview

- **Don't ask all questions at once.** One topic at a time. Let the user talk.
- **Follow the thread.** If they mention something interesting, dig deeper before moving on.
- **It's OK to skip blocks.** If the user has no decisions yet or no infrastructure details — skip and move on.
- **Short blocks for small projects.** A simple CRUD service might need 10 minutes. A complex distributed system might need an hour.
- **Let the user correct you.** After writing each file, they might realize they forgot something or said something wrong. That's the point.
- **Global artifacts are written once.** On subsequent milestones, they're read-only context unless the user explicitly says something changed.

## Cleanup

After the skill completes:
1. Run `hlv check` to validate the project structure. If there are errors — fix them before finishing.
2. Suggest the user run `/clear` to free up context window before the next skill.
