# Plan: P2-M014 — Safe review and integration workflow

Status: Complete
Milestone: P2-M014
Created: 2026-09-20

## Goal

Close the operator workflow after an agent run by providing a bounded review projection and an
explicit, audited, serialized integration command. A task branch must be reviewable and mergeable
without requiring the operator to construct ad hoc Git commands or bypass AgentForge's approval,
worktree, and evidence boundaries.

## Context

P2-M013 proved that Codex- and Claude-style terminal agents can run safely in a task-owned PTY.
After a successful run, AgentForge currently exposes task inspection, acceptance, and conservative
worktree retirement, but it does not provide a first-class branch diff or protected-branch
integration operation. The scheduler already models serialized integration reservations; this
milestone supplies the operator-facing boundary that consumes one accepted task at a time.

## Non-goals

- Do not make an agent, daemon, HUD, or task accept operation perform an integration implicitly.
- Do not add automatic conflict resolution, force pushes, rebases, resets, or destructive cleanup.
- Do not merge dirty worktrees, dirty target repositories, unresolved Git operations, detached
  targets, stale source heads, or unrelated branches.
- Do not introduce shell interpolation, ambient executable lookup, network remotes, pull requests,
  hosted review systems, or deployment behavior.
- Do not change the existing task-state meanings, captured/cooked/PTY execution modes, daemon
  protocol, scheduler batch semantics, intake format, or release workflow.
- Do not retire a worktree automatically after integration; review, retirement, and branch
  deletion remain explicit operator decisions.

## Proposed user-facing contract

Add direct operator commands:

```text
forge task diff <root> <task-id>
forge task integrate <root> <task-id> --target <branch> --actor <actor-id>
```

`task diff` is read-only. It validates task/worktree ownership, reports the source branch, source
head, target branch, merge base, changed-file count, bounded name/status output, and whether the
source worktree is clean. It must fail closed for missing or ambiguous managed worktrees and must
never create an audit record or mutate Git.

`task integrate` is an operator-only mutation. It requires a succeeded task, an explicit
`merge_protected_branch` approval recorded for that task, the `merge_protected_branch` capability,
a clean verified source worktree, a clean verified target worktree, no unresolved Git operation,
an exact source head matching the reviewed diff, and an explicit target branch. It acquires a
project-local integration lock, verifies the source is a fast-forward descendant of the target,
executes literal `git merge --ff-only` arguments, verifies the target head, records an integrity-
linked integration event, and releases the lock. Repeating an already-integrated operation is
idempotent and reports the existing target head without creating a second merge.

Integration never changes task acceptance automatically and never deletes the task branch. A
failed integration preserves the task, branch, worktree, and bounded failure evidence for repair
forward.

## Architecture

- Extend `WorktreeManager` with provider-neutral, direct-argument Git operations for bounded diff
  inspection, target-branch inspection, exact merge-base/source-head checks, and fast-forward-only
  integration. Keep all Git subprocess ownership and path validation in the worktree crate.
- Add a bounded `WorktreeDiff`/integration report that contains source and target identities,
  merge-base, changed-file summary, and target-head verification without returning unbounded patch
  data. The CLI may offer an explicit bounded `--patch` extension only if it remains within the
  existing evidence limit; the default command is name/status oriented.
- Add a cross-platform create-new integration lock under `.forge/`, with an RAII guard and bounded
  stale-lock diagnostics. Concurrent integration attempts must serialize or fail closed; no lock
  stealing is allowed.
- Add a versioned `IntegrationRecorded` audit event with source branch, source head, target branch,
  target before/after, merge base, outcome, and actor fields. Existing audit chains and unknown
  event rejection remain intact.
- Reuse `agentforge-operator` approval and task-state loading so integration cannot be invoked from
  a HUD, by an agent result, or without explicit human evidence.
- Keep daemon execution unchanged. A future daemon integration API, if needed, must be separately
  planned because it would expand serialized mutation and protocol authority.

## Safety invariants

- Every precondition is checked before the integration lock is acquired or Git is mutated.
- Source and target paths are canonical, repository-owned, and not the same linked worktree.
- The source branch and exact source head are verified immediately before merge; a stale review is
  rejected rather than silently recomputed.
- Only `git merge --ff-only` with literal arguments is permitted. No shell, force flag, reset, or
  cleanup command is introduced.
- The lock is released on success and every handled failure. A lock owned by another live attempt
  is reported; stale metadata is not overwritten automatically.
- Integration evidence is appended only after target-head verification. A failed Git operation
  records bounded failure evidence without claiming task acceptance.
- Existing worktree retirement continues to require a clean, unambiguous managed worktree and
  preserves the task branch.

## Expected file boundary

Implementation may modify only:

- `.plans/P2-M014-safe-review-integration.plan.md` (approval/completion metadata);
- `crates/agentforge-worktree/src/lib.rs` and `crates/agentforge-worktree/tests/worktree_manager.rs`;
- `crates/agentforge-audit/src/lib.rs` and `crates/agentforge-audit/tests/audit_store.rs`;
- `crates/agentforge-operator/src/lib.rs` and focused operator integration tests;
- `crates/agentforge-cli/src/main.rs` and focused CLI integration tests;
- `docs/adr/ADR-0026-safe-review-and-integration.md`;
- `docs/ORCHESTRATION.md`, `docs/APPROVAL_BOUNDARIES.md`, and focused README command examples;
- `docs/MILESTONES.md`, `PROJECT_STATE.md`, and `AGENT_HANDOFF.md` only during closure evidence.

No new dependencies, daemon protocol changes, scheduler changes, worktree policy weakening,
release workflow changes, or unrelated workspace files are authorized.

## Test-first matrix

| Case | Expected evidence |
| --- | --- |
| Read-only diff | Deterministic source/target/base and bounded name/status output; no audit or Git mutation beyond inspection. |
| Missing, dirty, ambiguous, or unrelated worktree | Diff and integration fail closed before mutation. |
| Missing approval/capability/task state | Integration is rejected before lock acquisition and merge. |
| Dirty or detached target | Integration is rejected without changing source or target. |
| Stale source head or non-fast-forward history | Integration is rejected with explicit evidence. |
| Successful fast-forward | Target head advances exactly to source head; branch and worktree remain available. |
| Repeated integration | Idempotent result with no second merge or duplicate success record. |
| Concurrent integration | One operation owns the lock; the other fails closed with bounded diagnostics. |
| Git failure/interrupt | Lock releases, target state is verified, failure evidence is durable, and no forced cleanup runs. |
| Cross-platform matrix | Worktree, audit, CLI, and lock tests pass on Linux, macOS, and Windows with Rust 1.85. |

## Implementation sequence

1. Confirm current Git fast-forward and lock APIs remain available on the supported matrix without
   adding dependencies.
2. Add bounded worktree diff/target inspection and integration primitives with ownership and exact
   head verification.
3. Add the versioned integration audit event and operator-side approval/state checks.
4. Wire `forge task diff` and `forge task integrate` with deterministic usage errors and bounded
   reports; leave daemon and HUD behavior unchanged.
5. Add fixture coverage for successful, stale, dirty, conflicting, repeated, concurrent, and
   failed integrations.
6. Update ADR and operator documentation, then run focused tests, the full gate, exact CI, and the
   closure evidence sequence.

## Failure classification

- Invalid task/approval/capability/ownership: controlled policy or usage failure; no Git mutation.
- Dirty, stale, detached, conflicting, or unresolved Git state: semantic integration rejection;
  preserve evidence and repair forward.
- Lock contention or stale-lock ambiguity: workflow/concurrency failure; fail closed without lock
  stealing.
- Git, filesystem, or platform API failure: infrastructure classification; verify target state and
  preserve the failure before any repair.
- Audit append or target verification failure: durable-state failure; never report success without
  both.

## Documentation impact

Document the review-to-integration sequence, required `merge_protected_branch` approval and
capability, fast-forward-only behavior, lock/concurrency semantics, idempotent retries, branch
preservation, and explicit retirement. Clarify that `forge task accept` remains independent from
integration and that daemon/HUD surfaces do not gain merge authority.

## Quality gates

- `cargo fmt --all -- --check`.
- `CARGO_TARGET_DIR=/tmp/agentforge-cargo-target ./scripts/gate.sh full`.
- Focused worktree, audit, operator, and CLI integration tests plus the complete workspace suite.
- `git diff --check` and repository policy checks.
- Exact-commit remote CI green across repository policy, stable, MSRV, CLI smoke, Ubuntu, macOS,
  and Windows.

## Acceptance criteria

- [x] Operators can inspect a bounded, deterministic task diff without mutation.
- [x] Integration requires explicit task state, capability, approval, target, ownership, cleanliness,
      and exact-head checks.
- [x] Only serialized, fast-forward-only, non-forced integration is possible through the command.
- [x] Integration success, idempotent replay, contention, and failure are audit-visible and bounded.
- [x] Task branches and worktrees remain preserved until explicit retirement.
- [x] Existing execution, daemon, HUD, scheduler, intake, and release behavior remains compatible.
- [x] Linux, macOS, and Windows tests cover the integration and lock behavior.
- [x] Documentation distinguishes review, acceptance, integration, and retirement boundaries.
- [x] Local gate, exact implementation CI, closure commit, and exact closure CI are recorded.

## Completion record

Implementation commit: `b0cbdc810a1835c9f27d9c064cd6646334fc4897`.
Exact implementation CI: `35548080732` — all seven jobs green across repository policy, stable,
MSRV, CLI smoke, Ubuntu, macOS, and Windows.
Closure commit: pending.
Exact closure CI: pending.
