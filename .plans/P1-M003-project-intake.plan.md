# Plan: P1-M003 — Project blueprint and task intake

Status: Draft
Milestone: P1-M003
Created: 2026-09-19

## Goal

Give an operator a durable, reviewable way to enter project goals, engineering guidelines, and
task-authority defaults, then validate those inputs and materialize explicit AgentForge task
contracts without requiring Rust code or direct manipulation of the binary task snapshot.

## Non-goals

- No TUI/HUD, graphical interface, daemon, remote service, or cloud-backed project registry.
- No natural-language interpretation, autonomous task decomposition, or model-generated authority.
- No automatic execution, worktree creation, approval granting, gate bypass, merge, deployment, or
  mutation of protected repository files.
- No replacement of `PROJECT_SPEC.md`, `AGENTS.md`, approved plans, or existing policy semantics.
- No new third-party Rust dependency without an explicit plan amendment and approval.

## Context

The current operator command can execute a task that already exists in
`.forge/state/tasks.snapshot`, but there is no supported user-facing path to author the project
intent or create that task contract. Project guidance is currently split across repository Markdown
and Rust-only construction of `AgentTask` values. P1-M003 supplies the missing intake boundary while
keeping durable files as the source of truth. The future P2 HUD can edit and observe the same
artifacts without introducing a second state model.

## Architecture placement

The intake layer sits between the operator and the existing state/policy/orchestration crates:

```text
operator files and CLI
        |
        v
blueprint/guideline validator
        |
        v
explicit task contract builder
        |
        +--> FileTaskStore (.forge/state/tasks.snapshot)
        +--> existing policy and orchestration
```

The blueprint is descriptive project context. The generated task contract remains the authority
boundary for execution. Validation must fail closed when required fields, identifiers, paths,
capabilities, approvals, or gates are malformed or ambiguous.

## Data flow

1. `forge init <root>` creates the project-local `.forge/` intake directory and bounded starter
   documents without overwriting existing files.
2. The operator edits the versioned blueprint and guideline files with a normal editor.
3. `forge blueprint validate <root>` parses and validates the documents, reporting deterministic
   field and line errors without mutating task state.
4. `forge task create <root> ...` requires validated intake, accepts explicit task-specific
   authority and dependencies, and writes a deterministic `TaskRecord` through `FileTaskStore`.
5. Existing `forge run` consumes only the resulting durable task state and continues to enforce
   policy, worktree, adapter, gate, audit, and review boundaries.

## Invariants

- Blueprint and guideline formats are versioned, bounded, deterministic, and human-editable.
- Initialization never overwrites an existing project or guideline file.
- Validation has no process, network, worktree, task-state, or approval side effects.
- Task creation never infers capabilities or approvals from prose; authority is explicit.
- Existing task IDs, revisions, dependency validation, and snapshot checksums remain authoritative.
- Equivalent input bytes produce equivalent normalized validation results and task records.
- Invalid or stale intake cannot replace a valid existing task snapshot.
- The CLI must not interpolate user input into shell commands.

## ADRs

- Add `docs/adr/ADR-0020-project-blueprint-and-intake.md` and register it in
  `docs/adr/README.md`.
- The ADR must explain why durable project files are the source of truth, why blueprint context is
  separate from executable task authority, and why the initial interface is CLI/editor based rather
  than a TUI.

## Public API / CLI

Add provider-neutral intake types behind a focused crate or existing state boundary, with no
provider-specific fields:

- versioned blueprint and guideline document types;
- deterministic parser/validator diagnostics with source location;
- explicit task creation input that maps only declared fields to `AgentTask`;
- read-only validation and non-overwriting initialization operations.

Add stable CLI commands:

```text
forge init <root>
forge blueprint validate <root>
forge task create <root> <task-id> [explicit task options]
```

The exact task options must be bounded and documented. A task cannot be created from free-form
prose alone.

## Compatibility analysis

Existing `.forge/state/tasks.snapshot`, `.forge/audit.log`, `forge run`, policy decisions, and
orchestration APIs remain compatible. The intake layer only produces existing domain contracts. A
future format version must reject unsupported versions rather than silently reinterpret them.

## Dependency analysis

Use the Rust standard library and existing workspace crates. Do not add a parser, serialization,
UI, or network dependency in this milestone. If a dependency becomes necessary, stop and amend the
plan before changing `Cargo.toml` or `Cargo.lock`.

## Expected file boundary

- `.plans/P1-M003-project-intake.plan.md`, `.plans/ACTIVE`
- `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, `docs/MILESTONES.md`
- `docs/BLUEPRINT.md`, `docs/ORCHESTRATION.md`
- `docs/adr/ADR-0020-project-blueprint-and-intake.md`, `docs/adr/README.md`
- `Cargo.toml`, `Cargo.lock`
- `crates/agentforge-core/**` or one new focused `crates/agentforge-intake/**`
- `crates/agentforge-state/**`
- `crates/agentforge-cli/**`
- focused unit and integration tests for parsing, validation, initialization, and task creation

No TUI, daemon, scheduler semantics, adapter semantics, persistence-format rewrite, workflow, hook,
or deployment changes without an amendment.

## Test-first matrix

| Behavior | Required evidence |
| --- | --- |
| Initialization | New files are created; existing files are preserved byte-for-byte |
| Parsing | Valid documents round-trip to deterministic normalized values |
| Validation | Missing, duplicate, malformed, oversized, and unsupported fields fail with stable diagnostics |
| Authority | Prose cannot grant capabilities, approvals, paths, or gates implicitly |
| Task creation | Explicit options produce a valid deterministic `TaskRecord` and snapshot |
| State safety | Existing snapshots are not replaced after validation or creation failure |
| CLI | Success, validation failure, existing-file conflict, and invalid task input have deterministic exit behavior |
| Integration | A generated task can be loaded by the current orchestration path |

## Implementation sequence

1. Approve this plan in a separate plan-only checkpoint and activate it only after P1-M002 closure
   state is recorded.
2. Define the versioned intake schema, limits, diagnostics, and starter templates in the ADR and
   `docs/BLUEPRINT.md`.
3. Implement parser/validator tests before the intake implementation.
4. Implement non-overwriting initialization and read-only validation.
5. Implement explicit task creation through the existing task/state contracts.
6. Add CLI wiring and an isolated temporary-repository integration fixture.
7. Run the full local gate and exact-head CI.
8. Close, merge, and verify post-merge CI with exact commit evidence.

## Failure modes

- Missing or invalid blueprint: stop before task-state mutation and report source locations.
- Unsupported schema version: reject explicitly; do not guess or migrate implicitly.
- Existing starter files: refuse overwrite and tell the operator which files are present.
- Invalid task authority: reject before writing the snapshot.
- Snapshot write failure: preserve the previous valid snapshot and return a classified error.
- Concurrent task creation: use the existing state-store boundary and reject stale revisions.

## Documentation impact

Document the file format, editing workflow, validation limits, explicit-authority rule, CLI
examples, and the boundary between blueprint context, task contracts, and runtime state. Update
milestone and handoff records only when the plan is activated or closed.

## Quality gates

- `./scripts/gate.sh full`
- focused intake and CLI tests
- MSRV 1.85.0 compatibility
- exact-head Repository policy, Stable code gate, MSRV, and CLI smoke CI
- no bypassed hooks or weakened validation

## Acceptance criteria

- [ ] An operator can initialize a project-local intake directory without overwriting files.
- [ ] An operator can edit and validate a versioned blueprint and guideline document.
- [ ] Validation is deterministic, bounded, read-only, and fail-closed.
- [ ] An operator can create an explicit task contract and durable task snapshot through the CLI.
- [ ] Generated tasks run through the existing orchestration path without authority expansion.
- [ ] Tests cover malformed input, conflicts, state preservation, and successful integration.
- [ ] Exact implementation, closure, merge, and post-merge CI evidence is recorded.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
