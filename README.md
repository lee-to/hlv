# hlv

[![Test](https://github.com/lee-to/hlv/actions/workflows/test.yml/badge.svg)](https://github.com/lee-to/hlv/actions/workflows/test.yml)
[![Lint](https://github.com/lee-to/hlv/actions/workflows/lint.yml/badge.svg)](https://github.com/lee-to/hlv/actions/workflows/lint.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**Specs first. Code second. Proof always.**

You define the *what*. LLMs generate the *how*. hlv validates the *proof*.

A compiled Rust binary that keeps LLM-generated code on a leash — validating every file, every milestone, every gate in seconds. Because prompts are not guarantees.

```
intent → artifacts → contracts → code → hlv check → proof
```

## The problem

You paste context into chat, hope the agent understood everything, then spend days chasing inconsistencies across files you barely reviewed.

LLMs generate code fast. But without machine-checkable invariants, you're left with:
- **No traceability** — which requirement produced which code?
- **No verification** — did the LLM actually follow the spec?
- **No gates** — what stops broken code from advancing?

## The solution: three layers, hard boundaries

HLV separates every project into three layers with strict ownership:

| Layer | Owner | Responsibility |
|-------|-------|----------------|
| **WHAT** | Human | Artifacts, constraints, milestones — everything the LLM needs, expressed in declarative files |
| **HOW** | LLM | All code. Every line. Generated from specs, disposable, regenerated on demand |
| **PROOF** | hlv | Machine-checkable validation. Every field, type, constraint, and invariant verified |

> Code is never the source of truth. Contracts are.

## What hlv does

hlv is a compiled Rust binary. It doesn't call the LLM. It doesn't generate code. It makes sure LLM output matches the spec.

```
$ hlv check

── Contracts ──
  ✓  14 contracts found, all valid
  ✓  acceptance criteria present on every contract
  ✖  contract/api-auth.md missing dependency link

── Traceability ──
  ✓  all code files traced to a contract
  ✓  no unlinked deliverables detected

── Status ──
  FAILED · 29/30 passed, 1 failure
```

### 30+ validations across 8 domains

| Domain | What it checks |
|--------|---------------|
| **Project map** | `project.yaml` structure and paths |
| **Contracts** | MD sections, YAML blocks, glossary refs, version alignment |
| **Test specs** | `derived_from` refs, unique IDs, gate coverage |
| **Traceability** | REQ → CTR → TST → GATE chains, no dangling refs |
| **Plan** | DAG without cycles, contract coverage |
| **Code traceability** | `@hlv` markers in code match contract rules |
| **LLM map** | every `map.yaml` entry exists on disk |
| **Constraints** | rule IDs, severity validation |

Phase-aware: checks expected at the current phase are automatically downgraded to info.

### Key commands

| Command | What it does |
|---------|-------------|
| `hlv init` | Scaffold the full HLV directory structure in seconds |
| `hlv check` | Run the full validation suite — specs, gates, deps, coverage |
| `hlv milestone` | Track progress across milestones |
| `hlv workflow` | See where you are and what the next step is |
| `hlv gates` | Enforce quality gates before milestone transitions |
| `hlv constraints` | Define cross-cutting rules as YAML configs |
| `hlv dashboard` | Full TUI with 5 tabs — Status, Contracts, Plan, Gates, Questions |
| `hlv trace --visual` | Visualize REQ → CTR → TST → GATE chains |
| `hlv plan --visual` | ASCII dependency graph with critical path |
| `hlv task` | Task lifecycle (start/done/block/unblock) |
| `hlv mcp` | Start MCP server (stdio or HTTP) |

All commands support `--json` for programmatic access.

## 5-minute quickstart

```bash
# 1. Create milestone
$ hlv milestone new add-payments
✓ Milestone created: add-payments

# 2. Capture intent (AI-driven interview)
$ /artifacts
✓ Intent captured: 8 decisions recorded

# 3. Generate specs
$ /generate
✓ Generated: contracts, plan (4 stages), gates-policy

# 4. Resolve questions, verify, check
$ /questions && /verify && hlv check
✓ All gates passed — ready to implement

# 5. Implement stage by stage
$ /implement
◐ stage 1/4 ████████████████████ ✓ validated
  stage 2/4 ████████████░░░░░░░░ implementing

# 6. Validate and ship
$ /validate
✓ All gates passed across all 4 stages

$ hlv milestone done add-payments
✓ Ready to ship.
```

## Why strictly typed languages

HLV is designed for **Rust, Go, TypeScript strict, Kotlin, Java** — languages where the compiler is another enforcement layer.

Each layer catches a different class of drift:
- **Contracts** catch requirement drift
- **Type system** catches structural drift
- **`@hlv` markers** catch traceability drift
- **`hlv check`** catches coverage drift
- **Validation gates** catch integration drift

> The compiler doesn't care that the LLM was pretty sure. Neither does hlv.

## Your best practices are LLM anti-patterns

| Developer pattern | LLM problem | HLV alternative |
|-------------------|-------------|-----------------|
| Dependency Injection | Magic wiring — LLM can't see what gets injected | Direct imports, explicit construction |
| Deep directory trees | Context burn — LLM wastes tokens navigating | Flat structure, everything discoverable |
| Convention over config | Magic paths — LLM guesses file locations | Explicit config, no guessing |
| Abstract factories | Hidden code paths — LLM can't trace what runs | Direct construction, visible types |
| ORM magic | Hidden SQL — LLM generates without seeing queries | Explicit queries, typed results |
| Lots of small files | Context hops — LLM loses coherence | Fewer, larger files with clear boundaries |

The `llm/` layer is designed for LLM generation: flat structure, explicit everything, direct code, errors as values, types as documentation.

> Code in llm/ is optimized for one reader: the next LLM invocation. And for one validator: hlv.

## How HLV compares

| Criteria | Chat + Copilot | Autonomous agents | **HLV** |
|----------|---------------|-------------------|---------|
| Spec approach | Ad-hoc prompts | Agent infers | **Formal contracts with invariants** |
| Who writes code | Human+AI mixed | Agent alone | **LLM writes all — within verified specs** |
| Verification | Manual review | Agent self-checks | **Independent Rust binary** |
| Context control | Entire codebase | Agent picks files | **One stage at a time** |
| Traceability | None | None | **REQ → contract → test → gate → code** |
| When LLM drifts | Hope you catch it | Hope agent catches itself | **hlv check shows what diverged** |
| Release criteria | "Looks good" | "Agent says done" | **All mandatory gates passed** |

## MCP integration

HLV includes a built-in [MCP](https://modelcontextprotocol.io/) server. Claude Code, web dashboards, and custom tools get full programmatic access to your project.

```json
{
  "mcpServers": {
    "hlv": {
      "command": "hlv",
      "args": ["mcp"]
    }
  }
}
```

```bash
# Or start SSE server for web clients
hlv mcp --transport sse --port 3000
```

12 resources + 27 tools + change notifications. See [docs/MCP.md](docs/MCP.md) for details.

## AI agent skills

Built-in skills drive the full development lifecycle:

| Skill | What it does |
|-------|-------------|
| `/artifacts` | Interview to capture domain context, stack, and constraints |
| `/generate` | Generate contracts, test specs, stages, and gates |
| `/implement` | Implement one stage at a time with validation cycles |
| `/validate` | Verify code against contracts, auto-create fix tasks on failure |
| `/verify` | Cross-check contracts for completeness and consistency |
| `/questions` | Surface open questions that block progress |

Skills are installed automatically by `hlv init`.

## Installation

<details>
<summary><b>macOS (Apple Silicon)</b></summary>

```bash
curl -fsSL https://github.com/lee-to/hlv/releases/latest/download/hlv-aarch64-apple-darwin.tar.gz | tar xz -C /usr/local/bin
```
</details>

<details>
<summary><b>macOS (Intel)</b></summary>

```bash
curl -fsSL https://github.com/lee-to/hlv/releases/latest/download/hlv-x86_64-apple-darwin.tar.gz | tar xz -C /usr/local/bin
```
</details>

<details>
<summary><b>Linux (x86_64)</b></summary>

```bash
curl -fsSL https://github.com/lee-to/hlv/releases/latest/download/hlv-x86_64-unknown-linux-gnu.tar.gz | tar xz -C /usr/local/bin
```
</details>

<details>
<summary><b>Linux (aarch64)</b></summary>

```bash
curl -fsSL https://github.com/lee-to/hlv/releases/latest/download/hlv-aarch64-unknown-linux-gnu.tar.gz | tar xz -C /usr/local/bin
```
</details>

<details>
<summary><b>Windows (x86_64, PowerShell)</b></summary>

```powershell
Invoke-WebRequest -Uri "https://github.com/lee-to/hlv/releases/latest/download/hlv-x86_64-pc-windows-msvc.zip" -OutFile hlv.zip
Expand-Archive hlv.zip -DestinationPath "$env:USERPROFILE\bin" -Force
```

Add `%USERPROFILE%\bin` to `PATH` if needed.
</details>

<details>
<summary><b>Build from source</b></summary>

```bash
git clone https://github.com/lee-to/hlv.git
cd hlv
cargo install --path .
```
</details>

Then bootstrap a project:

```bash
hlv init --project my-service --owner backend-team
hlv check
```

## Project structure

```
my-project/
├── project.yaml              # Project configuration
├── gates-policy.yaml          # Quality gates
├── map.yaml                   # File → contract traceability
├── human/
│   ├── glossary.yaml          # Domain glossary
│   ├── artifacts/             # Global artifacts (shared)
│   │   ├── context.md
│   │   ├── stack.md
│   │   └── constraints.md
│   └── milestones/
│       └── 001/
│           ├── plan.md        # Implementation plan
│           ├── stage_1.md     # Stage details
│           ├── contracts/     # Formal specifications
│           └── artifacts/     # Milestone-specific context
└── milestones.yaml            # Milestone & task tracking
```

## Architecture

See [docs/ARCH.md](docs/ARCH.md) for the full architecture description.

## Development

```bash
cargo build
cargo test
cargo clippy
cargo fmt --check
```

## License

[MIT](LICENSE)
