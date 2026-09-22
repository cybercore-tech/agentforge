# Plan: P4-M003 — Deterministic remote dispatch planning

Status: Approved
Milestone: P4-M003
Created: 2026-09-21
Owner: AgentForge project

## Goal

Connect the validated task graph, remote-worker descriptors, and durable-capable `LeaseBook` at
the scheduling boundary. A local authority will be able to plan one deterministic remote dispatch
batch for ready tasks, assigning each task to an eligible worker while preserving path ownership,
worker concurrency, lease ownership, and generation semantics.

## Non-goals

- No network transport, authentication, worker registration protocol, cloud mutation, or remote
  command execution.
- No daemon or CLI integration and no change to local process execution.
- No task contract redesign or remote capability negotiation; P4-M003 only schedules the existing
  validated task records against supplied worker descriptors.
- No automatic clock reads; the caller supplies the observation time and each lease window.
- No partial lease-book mutation when the complete requested batch cannot be planned.

## Context

P4-M001 established transport-neutral worker and lease semantics. P4-M002 made lease state durable
and explicit across restart. The remaining local seam before a future transport is a deterministic
admission decision: ready tasks must be selected in canonical order, path conflicts must remain
fail-closed, workers must be considered in canonical order, and a failed batch must not consume
leases before the caller can retry or inspect the failure.

## Scope

- Extend `agentforge-scheduler` with typed remote-dispatch requests, assignments, and batches.
- Add a `plan_remote_dispatch` function that:
  - validates worker identity uniqueness;
  - derives readiness and path ownership from the existing `TaskGraph`;
  - orders requested tasks and workers deterministically;
  - expires due leases only on the cloned planning book using the caller's observation time;
  - uses `LeaseBook::grant` to enforce ownership, concurrency, windows, and generations; and
  - commits the resulting lease book only after every requested assignment succeeds.
- Return assignment evidence containing task ID, worker ID, lease ID, and generation without
  opening a process or network connection.
- Add focused scheduler tests for deterministic assignment, worker capacity, path conflicts,
  readiness, lease generations, duplicate inputs, explicit expiry, and all-or-nothing failure.
- Document the local dispatch boundary and add ADR-0036.

## Architecture placement

`agentforge-core` remains authoritative for task and lease invariants. `agentforge-scheduler` owns
the deterministic cross-domain admission decision. `agentforge-state` remains responsible only
for persistence; callers may save the committed lease book after a successful plan. No scheduler
API will infer identity, authority, or transport reachability from a worker descriptor.

## Data flow

1. Caller supplies a validated graph, worker descriptors, lease-book state, dispatch requests, and
   `observed_at_ms`.
2. Scheduler validates unique workers, requested task readiness, duplicate request identities, and
   the existing path-ownership batch.
3. Scheduler clones the lease book and expires due active leases at the supplied observation time.
4. Requests and workers are considered in canonical task-ID and worker-ID order; `LeaseBook::grant`
   accepts each assignment and produces the next task generation.
5. On full success, the cloned lease book replaces the caller's book and the assignment batch is
   returned. On any failure, the caller's book is unchanged.

## Invariants

- Only ready tasks may be dispatched.
- Existing task path ownership remains fail-closed and deterministic.
- A task receives at most one active lease, and a worker never exceeds its declared concurrency.
- Worker and request ordering does not change the resulting assignment batch.
- Expiry uses only the supplied observation time.
- Failed planning does not mutate the original lease book.
- The scheduler creates no process, socket, file, cloud, or credential side effect.

## ADRs

- Add ADR-0036 documenting deterministic local dispatch planning and all-or-nothing lease mutation.

## Public API / CLI

- Add remote dispatch request, assignment, and batch types plus `plan_remote_dispatch` to
  `agentforge-scheduler`.
- Add no CLI, daemon, network, or persistence API in this milestone.

## Compatibility analysis

Existing `plan_batch`, `reserve_integrator`, task lifecycle, lease persistence, daemon protocol, and
CLI behavior remain unchanged. The new scheduler API is opt-in and consumes already validated
domain values.

## Dependency analysis

No new dependency. The implementation uses existing `agentforge-core` types and standard-library
ordering/collections.

## Expected file boundary

- `.plans/P4-M003-remote-dispatch-planning.plan.md`;
- `.plans/ACTIVE`;
- `crates/agentforge-scheduler/src/lib.rs`;
- `docs/REMOTE_WORKERS.md`;
- `docs/adr/ADR-0036-remote-dispatch-planning.md`;
- closure evidence in `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, and `docs/MILESTONES.md`.

## Test-first matrix

- Equivalent worker/request insertion orders produce identical assignments.
- Ready tasks dispatch; pending or dependency-blocked tasks fail closed.
- Overlapping task ownership fails before lease-book mutation.
- Worker concurrency limits and existing active leases are enforced.
- Expired leases are reclaimed only at the caller-supplied observation time.
- Successful dispatch advances task generations and returns assignment evidence.
- Duplicate workers, duplicate tasks, and duplicate lease IDs fail closed.
- A later worker-capacity or lease failure leaves the original lease book unchanged.
- No transport or process side effect is introduced.

## Implementation sequence

1. Commit this draft plan and validate plan-only repository policy.
2. Approve and activate the plan in a separate checkpoint.
3. Add scheduler request/assignment types and all-or-nothing planning.
4. Add deterministic and failure-path tests.
5. Update remote-worker documentation and add ADR-0036.
6. Run formatting, workspace checks, Clippy, tests, and the full repository gate.
7. Close the milestone in a separate documentation checkpoint with exact implementation and
   validation evidence.

## Failure modes

- Invalid graphs and path conflicts return existing structured scheduler failures.
- Invalid worker or lease input returns structured remote-worker or request errors before commit.
- Worker exhaustion returns a structured scheduling failure without mutating the caller's book.
- Lease conflicts and stale generations remain core state-machine errors.
- Test or gate failures must be classified before repair; no hook or validation is bypassed.

## Documentation impact

`docs/REMOTE_WORKERS.md` will document the local dispatch planner and its no-transport boundary.
ADR-0036 will record why planning is deterministic and why lease mutation is all-or-nothing.

## Quality gates

- `./scripts/gate.sh full`;
- deterministic scheduler and failure-path tests;
- no new dependency or external side effect; and
- repository policy validation with an approved plan in `HEAD` before implementation.

## Acceptance criteria

1. Ready task requests can be deterministically assigned to supplied remote workers.
2. Path ownership, worker concurrency, lease ownership, expiry, and generation invariants remain
   enforced by existing validated domain state.
3. A failed batch leaves the caller's lease book unchanged.
4. Assignment evidence identifies task, worker, lease, and generation.
5. Existing local scheduler, task, state, daemon, and CLI behavior remains compatible.
6. Documentation and ADR define the local, transport-neutral dispatch boundary.
7. The full repository gate passes for implementation and closure checkpoints.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
