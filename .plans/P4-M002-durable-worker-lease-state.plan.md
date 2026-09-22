# Plan: P4-M002 — Durable remote-worker lease state

Status: Draft
Milestone: P4-M002
Created: 2026-09-21
Owner: AgentForge project

## Goal

Persist the P4-M001 remote-worker lease state in a separate, checksummed project-local snapshot so
lease ownership and generations survive a daemon or process restart. Recovery remains explicit:
loading restores the last recorded state, and the caller supplies an observation time to expire
leases that are no longer valid.

## Non-goals

- No network transport, authentication, cloud mutation, or remote command execution.
- No daemon protocol or CLI integration; callers only receive a storage API in this milestone.
- No migration of the existing task snapshot or changes to task lifecycle semantics.
- No automatic wall-clock reads during load; recovery must remain deterministic and testable.
- No lease signing, identity verification, or distributed consensus.

## Context

P4-M001 added a validated worker descriptor and deterministic in-memory `LeaseBook`, but explicitly
deferred durable storage and recovery. Without persistence, a process restart loses the lease
generation and ownership history needed to reject stale renewals and safely reclaim tasks. The
existing `agentforge-state` crate already owns bounded, checksummed, atomic project-local snapshot
storage and is the correct boundary for a separate lease snapshot.

## Scope

- Extend `agentforge-core::remote` with validated lease restoration and deterministic lease-record
  iteration needed by a storage adapter.
- Add `LeaseStore` and `FileLeaseStore` to `agentforge-state` using a separate
  `.forge/state/remote-leases.snapshot` file and a versioned magic/schema header.
- Encode worker/lease/task identities, generation, timestamps, and terminal/active state with the
  existing bounded string/list primitives and checksum/atomic-replace pattern.
- Restore domain values through their validators and reject corrupt, truncated, oversized,
  unsupported-version, duplicate-active-task, and invalid-generation data before returning state.
- Document restart recovery as `load -> expire_due(observed_at_ms) -> continue` and preserve the
  local authority boundary.

## Architecture placement

`agentforge-core` owns lease invariants and restoration. `agentforge-state` owns bytes, paths,
checksums, atomic replacement, and format errors. The task snapshot remains independent so lease
schema changes cannot silently rewrite task state.

## Data flow

1. `FileLeaseStore::save` validates a `LeaseBook`, encodes deterministic lease records, checksums
   them, and atomically replaces `.forge/state/remote-leases.snapshot`.
2. `FileLeaseStore::load` validates file size, header, payload length, checksum, and record bounds.
3. Each persisted identity is parsed through the core remote-worker validators.
4. The reconstructed `LeaseBook` validates uniqueness, active-task exclusivity, state, generation,
   and lease windows.
5. The caller invokes `expire_due(observed_at_ms)` before scheduling or renewing work.

## Invariants

- Lease persistence uses a distinct magic and schema version from task persistence.
- Saves never partially replace the prior snapshot; temporary files are cleaned on failure.
- Missing lease state is valid and returns `None`.
- Invalid or incompatible bytes fail closed before constructing a returned lease book.
- Active leases retain their recorded expiry and generation across restart.
- Terminal leases remain terminal and continue to reserve their identity.
- No system clock or network is consulted by load/save.
- Task state and lease state are stored in separate files and APIs.

## ADRs

- Add ADR-0035 documenting the separate lease snapshot and explicit restart-recovery sequence.

## Public API / CLI

- Add `LeaseStore` and `FileLeaseStore` to `agentforge_state`.
- Add `DEFAULT_LEASE_RELATIVE_PATH` for `.forge/state/remote-leases.snapshot`.
- Add no CLI or daemon command.

## Compatibility analysis

Existing `TaskStore`, task snapshots, orchestrator paths, daemon protocol, and CLI behavior remain
unchanged. The new snapshot has its own magic/version and can be absent on projects created before
P4-M002. Existing callers do not need to load lease state unless they opt into remote-worker work.

## Dependency analysis

No new dependency. The implementation uses existing core types, standard-library collections, and
the established bounded binary snapshot mechanics.

## Expected file boundary

- `.plans/P4-M002-durable-worker-lease-state.plan.md`;
- `.plans/ACTIVE`;
- `crates/agentforge-core/src/remote.rs`;
- `crates/agentforge-state/src/lib.rs`;
- `docs/REMOTE_WORKERS.md`;
- `docs/adr/ADR-0035-durable-worker-lease-state.md`;
- closure evidence in `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, and `docs/MILESTONES.md`.

## Test-first matrix

- Lease-book restoration preserves deterministic record order and all lease fields.
- Restored active leases can be expired explicitly after a simulated restart; no clock is read by
  load.
- File round-trip preserves active, released, and expired leases.
- Snapshot bytes are deterministic independent of insertion order.
- Missing state returns `None`, and later saves atomically replace the prior snapshot.
- Bad magic, unsupported version, checksum mismatch, truncation, trailing bytes, oversized payload,
  invalid strings, invalid state tags, duplicate active tasks, and zero generations fail closed.
- A corrupted lease snapshot never mutates or invalidates the task snapshot.

## Implementation sequence

1. Commit this draft plan and validate plan-only repository policy.
2. Approve and activate the plan in a separate checkpoint.
3. Add core lease iteration/restoration and tests.
4. Add the bounded `LeaseStore`/`FileLeaseStore` format and tests.
5. Update remote-worker documentation and add ADR-0035.
6. Run formatting, workspace checks, Clippy, tests, and the full repository gate.
7. Close the milestone in a separate documentation checkpoint with exact implementation and
   validation evidence.

## Failure modes

- Filesystem failures return `StateError::Io` without replacing the previous snapshot.
- Format corruption returns a structured format error before allocation beyond configured bounds.
- Invalid restored domain values return a structured remote-worker state error.
- Lease conflicts and stale generations remain core state-machine errors after recovery.
- Test or gate failures must be classified before repair; no hook or validation is bypassed.

## Documentation impact

`docs/REMOTE_WORKERS.md` will document the snapshot path, format boundary, and deterministic
restart sequence. ADR-0035 will record why lease state is separate from task state and why expiry
requires an explicit caller-supplied timestamp.

## Quality gates

- `./scripts/gate.sh full`;
- deterministic state round-trip and corruption tests;
- no new dependency or network side effect; and
- repository policy validation with an approved plan in `HEAD` before implementation.

## Acceptance criteria

1. Remote lease state can be saved and loaded through a separate validated `FileLeaseStore`.
2. Restart recovery preserves ownership, generation, expiry, and terminal state without reading a
   clock or contacting a network.
3. Corrupt, incompatible, oversized, and semantically invalid snapshots fail closed.
4. Existing task snapshots and all current task/daemon/CLI behavior remain compatible.
5. Documentation and ADR define the explicit load-then-expire recovery boundary.
6. The full repository gate passes for implementation and closure checkpoints.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
