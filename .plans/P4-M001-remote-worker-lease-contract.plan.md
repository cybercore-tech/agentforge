# Plan: P4-M001 — Remote worker contract and lease foundation

Status: Draft
Milestone: P4-M001
Created: 2026-09-21
Owner: AgentForge project

## Goal

Establish the first safe distributed-execution boundary for AgentForge: a versioned,
transport-neutral remote-worker contract and a deterministic task-lease state machine. The
milestone makes worker identity, capabilities, lease ownership, renewal, expiry, and release
semantics explicit before any network transport or remote command authority is introduced.

## Non-goals

- No network client, server, cloud API, Mission Control mutation, or daemon transport.
- No remote command execution, shell transport, file transfer, reverse tunnel, or automatic
  worktree mutation.
- No replacement of local task, policy, approval, gate, audit, or integration authority.
- No durable on-disk lease store; persistence and recovery will be a later bounded milestone.
- No cryptographic identity, signature, or authentication scheme; the contract records the
  boundary that a later authenticated transport must enforce.

## Context

Phase 3 established an observation-oriented Mission Control plane and an outbound local connector.
AgentForge remains the local authority for task state and execution. Phase 4 is reserved for remote
workers and distributed execution, but no worker-dispatch contract currently exists in this
repository. The first increment must therefore define the protocol-independent domain semantics
without accidentally granting a remote system execution authority.

## Architecture placement

The contract belongs in `agentforge-core` because it is domain state shared by future transports,
the daemon, policy checks, and operator surfaces. It must not depend on sockets, HTTP, Cloudflare,
serialization crates, or process execution. The lease book is an in-memory deterministic state
machine suitable for unit tests and future durable/transport adapters.

## Data flow

1. A caller validates a bounded worker descriptor containing a stable worker ID, protocol version,
   platform label, capability set, and concurrency limit.
2. Local orchestration proposes a lease for an eligible task and worker.
3. The lease book accepts one active lease per task and records its owner, generation, and expiry.
4. The owning worker may renew its lease before expiry; unrelated workers and stale generations
   are rejected.
5. The local authority may release a lease or expire it at an explicit observation time.
6. A future transport can encode these values, but this milestone exposes no transport side effect.

## Invariants

- Worker IDs, platform labels, lease IDs, and task IDs are non-empty and bounded.
- Protocol version mismatches fail closed.
- Capability lists are deterministic, duplicate-free, and bounded.
- A worker's concurrency limit is non-zero and bounded.
- Lease expiry is strictly after issue time and bounded by the protocol maximum.
- At most one active lease exists for a task in one lease book.
- Renewals require the matching worker and lease generation and cannot extend an expired lease.
- Release and expiry are explicit state transitions; no background thread or clock is used.
- The state machine does not execute commands, persist secrets, or mutate worktrees.

## ADRs

- Add ADR-0034 documenting the transport-neutral remote-worker/lease boundary and its local-
  authority constraints.

## Public API / CLI

- Add `agentforge_core::remote` with validated worker descriptors, capability values, lease values,
  lease status, lease-book operations, and structured errors.
- Export the module from `agentforge-core`.
- No CLI or daemon command is added in this milestone.

## Compatibility analysis

Existing task, adapter, daemon, state, policy, scheduler, and audit APIs remain unchanged. The new
module references the existing `TaskId` but does not alter task-state transitions. The protocol
version is explicit so future wire implementations can reject unsupported versions safely.

## Dependency analysis

No new dependency. The implementation uses the Rust standard library and existing core types only.

## Expected file boundary

- `.plans/P4-M001-remote-worker-lease-contract.plan.md`;
- `.plans/ACTIVE`;
- `crates/agentforge-core/src/lib.rs`;
- `crates/agentforge-core/src/remote.rs`;
- `docs/REMOTE_WORKERS.md`;
- `docs/adr/ADR-0034-remote-worker-lease-boundary.md`;
- closure evidence in `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, and `docs/MILESTONES.md`.

## Test-first matrix

- Worker and lease identifiers reject empty, oversized, and control-character values.
- Unsupported protocol versions, invalid platforms, empty capabilities, duplicate capabilities,
  and invalid concurrency limits fail closed.
- Valid descriptors normalize capability order deterministically.
- Lease creation rejects invalid time windows, duplicate task ownership, and unsupported worker
  protocol versions.
- Renewal rejects the wrong worker, wrong generation, expired lease, and invalid extension while
  accepting a valid bounded renewal.
- Release and expiry produce deterministic terminal states and allow a later lease after expiry.
- Multiple workers and tasks remain isolated, and the lease book exposes deterministic active views.

## Implementation sequence

1. Commit this draft plan and validate the plan-only repository policy.
2. Approve the plan in a separate checkpoint and activate it through `.plans/ACTIVE`.
3. Add the core remote-worker value types and lease state machine with unit tests.
4. Add the protocol/authority documentation and ADR.
5. Run formatting, workspace checks, Clippy, tests, and the full repository gate.
6. Record exact implementation and validation evidence, then close the milestone in a separate
   documentation checkpoint.

## Failure modes

- Invalid worker or lease input returns a structured validation error without state mutation.
- Lease conflicts return the existing ownership conflict without replacing the active lease.
- Stale or unauthorized renewal/release attempts fail without changing the lease.
- Unsupported future protocol versions fail closed.
- Test or gate failures must be classified before repair; no hook or validation is bypassed.

## Documentation impact

`docs/REMOTE_WORKERS.md` will define the contract, lifecycle, authority boundary, and explicit
future work. ADR-0034 will record why the first P4 increment is transport-neutral and observation-
only. The closure checkpoint will update milestone and handoff evidence without claiming remote
execution or production distribution.

## Quality gates

- `./scripts/gate.sh full`;
- exact implementation commit validation;
- no new dependency or network side effect; and
- repository policy validation with an approved plan in `HEAD` before implementation.

## Acceptance criteria

1. `agentforge-core` exposes a documented, versioned remote-worker descriptor and lease contract.
2. The lease state machine deterministically enforces ownership, generation, expiry, and release.
3. Focused unit tests cover valid paths and fail-closed negative paths from the test-first matrix.
4. Documentation and ADR state that no network or remote execution authority exists yet.
5. The full repository gate passes for the implementation and closure checkpoints.
6. Closure evidence records the exact implementation commit and validation result.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
