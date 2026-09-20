# Plan: P2-M009 — Daemon audit-sequence continuation

Status: Complete
Milestone: P2-M009
Created: 2026-09-20

## Goal

Make persisted daemon execution continue an existing project audit chain instead of rejecting the
first new event with `audit sequence expected 1, got N`. A daemon run must append contiguous,
integrity-linked events after prior direct runs and operator transitions while preserving the
existing task, adapter, audit, and acceptance boundaries.

## Why now

Real operator dogfooding produced a deterministic failure on `/home/raven/.sysops/omniscient`:
the project already contained five valid audit records, then `forge daemon run` computed the next
sequence as `6` but built a fresh in-memory audit log that only accepted sequence `1`. The daemon
rejected `P1-M003-T0003` before launching the agent. The task and clean managed worktree remain
available as a reproduction fixture.

## Scope

1. **Audit continuation** — preserve the persisted tail sequence when constructing the bounded
   execution audit for `execute_process_persisted` and the daemon call path.
2. **Integrity preservation** — keep append-only framing, previous-digest chaining, contiguous
   sequence validation, and existing corruption failures unchanged.
3. **Regression evidence** — cover daemon/persisted execution after a non-empty audit history,
   including successful append and failure-before-launch behavior.
4. **Operator documentation** — document the corrected continuation behavior and the reproduction
   evidence without claiming broader daemon guarantees.

## Non-goals

- No audit format or wire-protocol version change.
- No truncation, deletion, repair, migration, or rewriting of existing audit records.
- No changes to task acceptance, approval boundaries, capability policy, worktree ownership, or
  adapter process semantics.
- No concurrency redesign, multi-daemon support, remote storage, or external dependencies.
- No changes to direct `forge run` behavior unless a regression test proves compatibility requires
  a shared correction.
- No implementation before this plan is separately approved and committed.

## Architecture placement

- `agentforge-audit` remains the append-only, integrity-checked persistence boundary.
- `agentforge-orchestrator` owns the persisted execution sequence handoff and emits local stage
  events for one attempt.
- `agentforge-daemon` continues to open the project audit store and invoke the existing persisted
  orchestration path; it does not gain independent audit semantics.
- CLI and HUD surfaces remain unchanged except for documentation or test evidence needed to expose
  the repaired path.

## Data flow

1. The daemon opens the existing `FileAuditStore` and reads its verified tail.
2. Persisted orchestration derives the next permitted sequence from that tail.
3. The attempt audit receives that sequence origin before its first append.
4. On success or bounded adapter failure, events append in order to the existing file chain.
5. A preflight failure before event creation leaves task state and audit state unchanged.
6. The operator can inspect the resulting chain and independently accept or retire the task.

## Invariants

- The first new event is exactly `last_sequence + 1`, or `1` for an empty log.
- No sequence gap, duplicate, or reset is introduced by daemon execution.
- Every new event links to the prior persisted digest and remains verifiable by the existing store.
- Existing records are never rewritten or discarded.
- A failed preflight does not launch the agent or create misleading execution events.
- Successful execution remains separate from task acceptance.
- The existing clean-worktree, capability, approval, and task-identity checks remain authoritative.

## ADRs

- `docs/adr/ADR-0014-event-audit-log.md`
- `docs/adr/ADR-0019-orchestration-loop.md`
- `docs/adr/ADR-0023-controlled-operator-actions.md`

## Public API / CLI

No new public CLI command or wire field is planned. Existing commands must continue to work:

```text
forge daemon run <root> <task-id> <absolute-executable>
forge task inspect <root> <task-id>
forge hud <root>
```

## Compatibility analysis

- Audit files written before this milestone remain readable and appendable.
- Empty audit logs retain first sequence `1` behavior.
- Existing direct-run and operator-action audit records remain valid.
- The standard-library-only and Rust MSRV constraints remain unchanged.
- Platform behavior must remain consistent across Linux, macOS, and Windows CI.

## Dependency analysis

No new crate or external dependency. Changes, if needed, stay inside the existing audit,
orchestrator, daemon, test, and documentation boundaries.

## Expected file boundary

- `.plans/P2-M009-daemon-audit-sequence-continuation.plan.md`
- `.plans/ACTIVE` only in the separate approval checkpoint
- `crates/agentforge-audit/src/lib.rs` only if a narrowly scoped sequence-origin API is required
- `crates/agentforge-orchestrator/src/lib.rs`
- `crates/agentforge-orchestrator/tests/` and/or existing daemon tests only for regression coverage
- `crates/agentforge-daemon/src/lib.rs` only if required to thread the existing persisted boundary
- `crates/agentforge-daemon/tests/` only for daemon-facing evidence
- `docs/DAEMON.md`, `docs/ORCHESTRATION.md`, and milestone/state/hand-off records during closure
- `Cargo.lock` only if an existing workspace change requires regeneration; no new dependencies

## Test-first matrix

| Area | Required evidence |
| --- | --- |
| Empty audit | First persisted daemon event still starts at sequence `1`. |
| Existing history | A daemon run appends after a pre-populated valid chain without sequence failure. |
| Integrity | Previous digest, contiguous sequence, and full-file verification remain valid. |
| Failure path | Preflight failure does not launch the executable or append false execution events. |
| Regression | Existing direct run, task lifecycle, HUD, and audit tests remain green. |
| Platform | Stable, MSRV, policy, CLI smoke, Linux, macOS, and Windows CI remain green. |

## Implementation sequence

1. Commit this draft as a plan-only checkpoint; do not activate it yet.
2. Approve the plan in a separate commit by adding `.plans/ACTIVE` and changing status to
   `Approved`.
3. Reproduce the existing-history failure in a bounded test fixture using the persisted audit
   store; preserve the real operator task as evidence, not as a mutable test fixture.
4. Implement the smallest sequence-origin correction behind the existing audit/orchestration
   boundary.
5. Run focused tests, then the full local gate and exact-head CI.
6. Re-run the real daemon task only after the approved implementation is validated; inspect audit,
   task, HUD, and worktree evidence before acceptance and retirement.
7. Close the milestone with separate evidence documentation and exact-SHA closure validation.

## Failure modes

- Existing audit corruption remains a hard failure; this milestone must not silently repair it.
- A sequence mismatch after the correction is a semantic/test failure requiring forward repair.
- Any digest mismatch, task-state mutation before launch, or duplicate event is a release blocker.
- CI or gate failures must be classified before editing and must not be bypassed.

## Documentation impact

Document that daemon execution appends to an existing verified audit chain and that prior audit
history is preserved. Record the real operator reproduction and exact validation evidence during
closure.

## Quality gates

- `CARGO_TARGET_DIR=/tmp/agentforge-cargo-target ./scripts/gate.sh full`
- Focused audit/orchestrator/daemon regression tests.
- Exact-head GitHub CI for implementation and closure commits.
- No hooks or validation may be bypassed.

## Acceptance criteria

- [x] A daemon run on a project with existing valid audit records appends successfully at the next
      contiguous sequence.
- [x] The resulting audit file passes existing integrity and sequence verification.
- [x] Empty-log behavior still starts at sequence `1`.
- [x] Preflight failures remain fail-closed without launching the agent or fabricating events.
- [x] Existing direct execution, task lifecycle, HUD, worktree, and cross-platform gates remain
      green.
- [x] The real `P1-M003-T0003` reproduction completes only after implementation validation and is
      independently reviewed before acceptance.

## Completion record

Implementation commit: `d4d17cd`.
Implementation CI: `35527442420` — all jobs green across repository policy, stable, MSRV, CLI
smoke, Ubuntu, macOS, and Windows for exact head `d4d17cd2ae88ae4504e3728f298dbdbc51acaa9c`.
Closure commit: pending.
Closure CI: pending.
Completed: 2026-09-20.
Notes: Real Omniscient dogfooding continued audit records from sequence 5 through 8; the no-op
fixture task was explicitly cancelled and its clean worktree retired with its branch preserved.
