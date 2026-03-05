# hlv

[![Test](https://github.com/lee-to/hlv/actions/workflows/test.yml/badge.svg)](https://github.com/lee-to/hlv/actions/workflows/test.yml)
[![Lint](https://github.com/lee-to/hlv/actions/workflows/lint.yml/badge.svg)](https://github.com/lee-to/hlv/actions/workflows/lint.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Contract-driven development CLI. Turns informal notes into formal contracts, validates implementation against them, and ensures every requirement is covered by tests.

```
your notes → formal contracts → validation → code → proof
```

## How it works

HLV enforces a structured development workflow:

1. **Artifacts** — gather domain context, tech stack, and constraints
2. **Contracts** — generate formal specifications from your notes
3. **Implementation** — build stage-by-stage with validation at each step
4. **Validation** — verify code against contracts, auto-create fix tasks

Each change is a **milestone** with its own contracts, stages, and tests. Global artifacts (glossary, constraints, architecture decisions) are shared across milestones.

## Quick start

### Install

#### macOS (Apple Silicon)

```bash
curl -fsSL https://github.com/lee-to/hlv/releases/latest/download/hlv-aarch64-apple-darwin.tar.gz | tar xz -C /usr/local/bin
```

#### macOS (Intel)

```bash
curl -fsSL https://github.com/lee-to/hlv/releases/latest/download/hlv-x86_64-apple-darwin.tar.gz | tar xz -C /usr/local/bin
```

#### Linux (x86_64)

```bash
curl -fsSL https://github.com/lee-to/hlv/releases/latest/download/hlv-x86_64-unknown-linux-gnu.tar.gz | tar xz -C /usr/local/bin
```

#### Linux (aarch64)

```bash
curl -fsSL https://github.com/lee-to/hlv/releases/latest/download/hlv-aarch64-unknown-linux-gnu.tar.gz | tar xz -C /usr/local/bin
```

#### Windows (x86_64, PowerShell)

```powershell
Invoke-WebRequest -Uri "https://github.com/lee-to/hlv/releases/latest/download/hlv-x86_64-pc-windows-msvc.zip" -OutFile hlv.zip
Expand-Archive hlv.zip -DestinationPath "$env:USERPROFILE\bin" -Force
```

Then add `%USERPROFILE%\bin` to `PATH` if needed.

### Build from source

```bash
git clone https://github.com/lee-to/hlv.git
cd hlv
cargo install --path .
```

### Bootstrap a project

```bash
hlv init --project my-service --owner backend-team
```

This creates the project structure with a first milestone. Then use AI agent skills (`/artifacts`, `/generate`, `/implement`, `/validate`) to drive the workflow.

## Commands

| Command | Description |
|---------|-------------|
| `hlv init` | Bootstrap project structure |
| `hlv status` | Project summary |
| `hlv check` | Validate all artifacts (`--watch` for live reload) |
| `hlv plan` | Show implementation plan (`--visual` for TUI) |
| `hlv trace` | Traceability map (`--visual` for TUI) |
| `hlv workflow` | Current phase and next actions |
| `hlv milestone` | Create, list, complete milestones |
| `hlv task` | Task lifecycle (start/done/block/unblock) |
| `hlv stage` | Stage metadata management |
| `hlv gates` | Quality gates CRUD and execution |
| `hlv constraints` | Rule-based constraints management |
| `hlv artifacts` | List and show artifacts |
| `hlv glossary` | Display project glossary |
| `hlv commit-msg` | Generate conventional commit messages |
| `hlv dashboard` | Interactive TUI dashboard |
| `hlv mcp` | Start MCP server (stdio or HTTP) |

All commands support `--json` for programmatic access.

## Validation (`hlv check`)

Runs multiple groups of checks:

1. **Project map** — `project.yaml` structure and paths
2. **Contracts** — MD sections, YAML blocks, glossary refs, version alignment
3. **Test specs** — `derived_from` refs, unique IDs, gate coverage
4. **Traceability** — REQ → CTR → TST → GATE chains, no dangling refs
5. **Plan** — DAG without cycles, contract coverage
6. **Code traceability** — `@hlv` markers in code match contract rules
7. **LLM map** — every `map.yaml` entry exists on disk
8. **Constraints** — rule IDs, severity validation

Phase-aware: warnings expected at the current phase are automatically downgraded to info.

Exit codes: `0` = ok, `1` = warnings, `2` = errors.

## MCP server

HLV includes a built-in [MCP](https://modelcontextprotocol.io/) server for AI agent integration:

```bash
# stdio transport (for Claude Code, etc.)
hlv mcp

# HTTP transport
hlv mcp --transport sse --port 3000
```

12 resources (read-only) + 27 tools (operations) + change subscriptions. See [docs/MCP.md](docs/MCP.md) for details.

## AI agent skills

HLV ships with agent skills that drive the development workflow:

| Skill | Description |
|-------|-------------|
| `/artifacts` | Interview to gather domain context and constraints |
| `/generate` | Generate contracts, test specs, and implementation plan |
| `/implement` | Implement stages with validation cycles |
| `/validate` | Verify code against contracts, create fix tasks |
| `/verify` | Cross-check contracts for completeness |
| `/questions` | Surface open questions for human decision |

Skills are installed automatically by `hlv init`.

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
