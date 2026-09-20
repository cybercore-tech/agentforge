# Plan: P0-M007 — Gate engine

Status: Draft
Milestone: P0-M007
Created: 2026-09-19

## Goal

Add a standard-library-only gate engine that runs explicit local checks and returns bounded,
structured evidence for later CI classification, audit, policy, diagnostics, and vertical-slice work.

## Non-goals

- No shell construction, user-facing gate CLI, task transition, adapter launch, CI API, audit
  persistence, policy enforcement, automatic repair, or external Rust dependency.

## Context

P0-M006 can execute an agent but does not validate its work. This milestone supplies local gate
evidence. Base main is `023a74969826f1de1bfb80c49e0dda125942336b`; P0-M006 post-merge CI
`35483102850` passed all four jobs for that exact commit.

## Architecture placement

New `agentforge-gate` owns explicit gate definitions, direct process execution, bounded capture,
deadlines, and reports. Existing core task semantics remain unchanged. A later orchestrator supplies
verified worktree paths; this milestone does not infer task ownership.

## Data flow

1. Caller supplies a named gate with an absolute executable, literal arguments, explicit
   environment, timeout, output limit, and existing working directory.
2. The runner validates configuration, clears the ambient environment, launches direct arguments,
   concurrently drains output, enforces limits/deadlines, and reaps the direct child.
3. It returns raw stdout/stderr, exit code, working directory, and exit/fail/timeout/output-limit
   outcome. Reports are evidence only and never accept tasks or initiate repair.

## Invariants

- Gate names are stable nonempty identifiers; duplicate names in a batch are rejected.
- Executables are absolute and no task-controlled text reaches a shell.
- Ambient Git overrides, credentials, and environment values are not inherited.
- Capture is bounded across both streams; direct children are reaped for every outcome.
- Batch report order follows input definition order.

## ADRs

Proposed ADR-0012 records explicit gate definitions and structured process evidence.

## Public API / CLI

- `GateDefinition`, `GateRunner`, `GateReport`, `GateOutcome`, and `GateError`.
- Single and ordered-batch execution APIs; no stable CLI in P0-M007.

## Dependency analysis

Add one workspace crate with no external dependencies.

## Expected file boundary

Plan and closure: `.plans/P0-M007-gate-engine.plan.md`, `.plans/ACTIVE`, `PROJECT_STATE.md`,
`AGENT_HANDOFF.md`, `docs/MILESTONES.md`, `docs/GATES.md`,
`docs/adr/ADR-0012-gate-engine.md`, and `docs/adr/README.md`.

Implementation: `Cargo.toml`, `Cargo.lock`, `crates/agentforge-gate/**`, and `docs/GATES.md`.

## Test-first matrix

| Behavior | Evidence |
| --- | --- |
| Direct arguments | Literal spaces and shell metacharacters reach fixture unchanged |
| Explicit environment | Fixture sees supplied values and no inherited Git index variable |
| Working directory | Fixture reports supplied canonical directory |
| Exit outcomes | Reports preserve exit code and raw output |
| Timeout/output flood | Direct child is reaped and capture stays bounded |
| Batch behavior | Input order is preserved and duplicate names fail |
| Invalid UTF-8 | Raw bytes are preserved without panic |

## Implementation sequence

1. Review this Draft plan and ADR-0012.
2. Commit Approved plan and active pointer separately from code; require local and remote green.
3. Add crate, fixture, definitions, runner, reports, and focused tests.
4. Run full local gate and Rust 1.85.0 focused tests.
5. Commit implementation, validate exact CI, close, merge, and post-merge validate before P0-M008.

## Acceptance criteria

- [ ] Approved plan is committed and green before implementation.
- [ ] Direct, explicit, bounded gate execution is implemented and tested.
- [ ] Ordered batch execution and duplicate rejection are tested.
- [ ] No external dependency or shell invocation is introduced.
- [ ] Exact implementation, closure, and post-merge CI are green.

## Completion record

Implementation commit: pending
Implementation CI: pending
Closure commit: pending
Closure CI: pending
Post-merge main: pending
Post-merge CI: pending
Completed: pending
