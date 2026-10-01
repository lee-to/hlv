# External execution evidence (v1)

A valid REQ → CTR → TST → GATE chain establishes structural traceability. It does
not establish that a test ran or that an old result applies to the current inputs.
HLV can optionally consume externally produced execution evidence. It does not
replace test runners, interpret arbitrary runner output, or authenticate a producer's
claim. A compatible `passed` result means the producer reports success for the exact
configured input snapshot; test quality and correctness still belong to the runner.

## Enable evidence for selected bindings

Add an optional section to `project.yaml`:

```yaml
execution_evidence:
  path: validation/execution-evidence.yaml
  bindings:
    - id: checkout-create
      requirement: REQ-ORDER-001
      approved_requirement_revision: approved-v1
      requirement_file: human/milestones/001/traceability.yaml
      test: CT-ORDER-CREATE-001
      gate: GATE-CONTRACT-001
      test_paths:
        - llm/tests/order.create/
        - human/milestones/001/test-specs/order.create.md
      code_paths:
        - llm/src/order.create/
        - llm/Cargo.toml
        - llm/Cargo.lock
```

`path` is relative to the HLV config root (`.hlv/` in adopted projects). All binding
input paths are relative to the repository root, including `requirement_file`:
use `.hlv/human/...`, `.hlv/llm/...`, and observed roots such as `src/` when adopting
an existing project. No absolute paths, parent traversal or symlinks are supported.
Keep the manifest and produced reports outside the declared input directories.
Directories recurse through every regular file, without gitignore filtering. Empty
input directories and missing/unreadable inputs are incompatible.

`requirement_file` must be a traceability YAML containing the requirement and a
mapping to the configured test and gate; the gate must exist in the current policy.
`approved_requirement_revision` is an explicit human-approved revision label, not
an approval HLV invents. Select test implementation AND specification inputs, and
all relevant source, contract, dependency and build inputs. Evidence freshness is
limited to this declared scope. Changes outside that scope do not invalidate it.
The requirement traceability file and gates policy are always included automatically.

Omitting the entire section preserves default validation behavior. An enabled
section requires at least one unique binding and exactly one current record per
binding. Missing records are errors. There is no inference from existing gate
statuses, Markdown reports, CI URLs, or an empty run list.

## Producer protocol

1. Run `hlv evidence snapshot --root <repo>` **before** running the tests. It prints
   `{schema_version: 1, snapshots: [...]}` as JSON and never runs tests or records
   an outcome. Save this output outside the declared input directories.
2. Run the existing external runner against those inputs and collect its actual
   outcome and observation/report artifact.
3. Capture a second snapshot and compare it to the first. If inputs changed while
   the runner executed, discard the run or publish `incomplete`; never attach a
   fresh snapshot to an older successful run.
4. Publish `schema_version: 1` and a `runs` list at the configured evidence path.
   Copy each original snapshot unchanged into the corresponding record. YAML or
   JSON syntax is accepted. Publish atomically after the report artifact is ready.

Example record (replace illustrative hashes with the snapshot command's output):

```yaml
schema_version: 1
runs:
  - snapshot:
      binding: checkout-create
      requirement: REQ-ORDER-001
      approved_requirement_revision: approved-v1
      test: CT-ORDER-CREATE-001
      gate: GATE-CONTRACT-001
      inputs:
        # Exact repo-relative file -> lowercase SHA-256 map from the snapshot.
        # Include EVERY entry, including the traceability file and gates policy.
        llm/src/order.create/service.rs: <64 lowercase hex characters>
    code_revision: <runner checkout commit or immutable build revision>
    run_id: <unique CI run/job/attempt identity>
    outcome: passed
    finished_at: '2026-01-01T00:00:00Z'
    artifact_ref: https://ci.example.invalid/runs/123/report
```

`code_revision` and `run_id` must be nonempty producer identities. Content hashes
are the compatibility authority, including dirty and untracked files; HLV does not
compare the revision label to Git HEAD. Raw bytes are hashed with SHA-256. File keys
use repository-relative paths with `/` separators. The exact map is compared, so
additions, deletions, renames, content changes, and scope changes that alter the
resulting file map are detected.
Requirement approval and requirement/test/gate identity must also match exactly.
Changing a binding ID requires a new record; an old unknown record is rejected.
The file is a current-run manifest, not a history store: replace previous records,
and do not keep two attempts for the same binding. Missing evidence is `missing`;
a readable older snapshot is `stale`; inputs that cannot be resolved are `invalid`.
There is no time-based expiry in v1.

Outcomes are `passed`, `failed`, `incomplete`, or `skipped`. Terminal records require
an RFC3339 `finished_at` and a nonempty `artifact_ref`. An incomplete record may omit
these. HLV references artifacts without fetching them; it does not parse their
contents or deduce success from them. Duplicate/unknown bindings, unsupported schema
versions, malformed hashes, missing terminal metadata and unknown fields are errors.
See `schema/execution-evidence-schema.json` for the evidence contract.

## Reports and gating

`hlv check` text/JSON and MCP `hlv_check` show `structural_status` separately from
`execution_evidence`. Evidence reports include per-binding status, reason, run ID,
code revision and artifact reference. The evidence aggregate is `passed` only when
all configured bindings have compatible passed records; otherwise it reports the
first non-passing binding in configuration order. `not_configured` is explicit.

Opt-in missing, stale, invalid, failed, incomplete or skipped evidence produces
blocking `EVD-*` diagnostics, even with `validation.strictness: relaxed`. Relaxed
mode skips command execution, not this deterministic compatibility check. Structural
status excludes evidence outcomes and gate/constraint command failures. Normal gate
commands retain their existing execution behavior; consuming external evidence does
not run a second test suite or manufacture records from gate exit codes.

`hlv status` reports evidence freshness without executing tests. Its Gates section
labels stored gate statuses as **last run; freshness not checked**. Those historical
statuses remain separate from versioned external evidence. Evidence errors do not
change the read-only status command's exit code; use `hlv check` for a blocking check.
Explicit diagnostic waivers still apply to check exit policy; the evidence status
continues to show the underlying outcome.

## Deterministic example and negative control

`tests/fixtures/execution-evidence-project` is a synthetic contract example, not a
claim of a real CI run. Its test observes `1` from `llm/src/value.sh`. The integration
negative control changes that output to `2`, verifies the structural traceability
chain remains valid, executes the test, and publishes its actual failed outcome.
The report then shows `structural_status: passed`, `execution_evidence.status: failed`,
and a nonzero check exit code. Separate tests cover stale/missing records, input
changes, malformed metadata, opt-out defaults, snapshot export and adopted layouts.
