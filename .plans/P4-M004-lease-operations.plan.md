# Plan: P4-M004 — Remote-worker leases in the CLI and daemon

Status: Approved
Milestone: P4-M004
Created: 2026-09-24
Owner: AgentForge project
Implementer: operator-authored (lease authority is a governance boundary)

## Goal

Make remote-worker leases an operator-visible, audited part of AgentForge instead of library-only
foundations. Operators register workers, grant, renew, release, and expire task leases from the
CLI. Every change is audited. A leased task cannot also be run locally. The daemon expires due
leases on its own, without colliding with executions. Nothing contacts a worker yet.

## Non-goals

- No transport, authentication, worker process, or remote execution (P4-M005 and later).
- No automatic dispatch: every grant is an explicit operator command.
- No change to task lifecycle states: a leased task stays `pending` and ready. The lease, not the
  task state, records the reservation.
- No HUD section for leases (a later operator-experience milestone).

## Context

P4-M001 to P4-M003 built:
- the worker descriptor and the `LeaseBook` state machine;
- the durable `FileLeaseStore` (`.forge/state/remote-leases.snapshot`);
- `plan_remote_dispatch`.

None of this is reachable from `forge` or `forged`, and nothing prevents a leased task from also
being launched locally. P1-M008 made approvals commit-bound, which a later milestone will use to
accept remote results by exact SHA.

Found while planning: `FileAuditStore` appends are not coordinated across handles. A long daemon
execution holds its own handle, so a second appender in the daemon would write a stale sequence
number. That is why the daemon's expiry sweep must share the execution slot (below). The existing
cross-process case (a CLI command appending while the daemon executes) is recorded as dogfooding
finding 9 for a separate milestone.

## Architecture placement

- **Worker profiles:** `.forge/workers/<worker-id>.conf`, bounded `key=value` like agent and gate
  profiles. The keys are `platform=`, repeated `capability=`, and `max_leases=`. Symlinks,
  oversized files, unknown keys, and invalid values fail closed. The file name is the worker ID.
  The profiles build `RemoteWorkerDescriptor`s. A worker ID is a label, not an identity:
  registration grants no authority.
- **Audit:** a new `AuditEventKind::LeaseRecorded` (code 12) with `action`
  (`granted|renewed|released|expired`), `lease_id`, `worker_id`, `generation`, and `expires_at_ms`,
  plus the task ID. It is additive; `forge` and `forged` from the same build are required, as with
  `IntegrationRecorded`.
- **`agentforge-operator` lease module:**
  - `list_workers`, `list_leases(now)`, `grant_lease(task, worker?, ttl, now, actor)`,
    `renew_lease`, `release_lease`, and `expire_leases(now, actor)`.
  - Every mutation takes an exclusive `.forge/state/remote-leases.lock`: create-new, removed on
    drop, a CLI waits up to 2 s, never stolen. It loads the book, applies the change, saves the
    snapshot, then appends audit events.
  - `grant_lease` goes through `plan_remote_dispatch` with the registered workers (or the one named
    worker). That enforces readiness, path ownership, capacity, one active lease per task, and
    generations. It refuses a task that is not `pending`.
  - Lease IDs are deterministic: `<task-id>.L<n>`, where `n` is the task's lease count plus 1.
  - Renew and release act for the recorded owner and generation. Until transport exists, the
    operator stands in for the worker.
  - The operator crate gains a dependency on `agentforge-scheduler`. It is internal: no external
    dependency and no registry change.
- **Local launch guard (orchestrator):** `forge run`, `task launch`, the daemon's `run` and
  `launch` (all through `execute_process_persisted` and `launch_process_persisted`) refuse a task
  with an active lease at the current time, before any side effect. `launch-batch` skips a leased
  task without blocking its siblings, and reports it. The guard only reads the lease snapshot.
- **CLI:**
  - `forge worker list <root>`;
  - `forge lease list <root>`, with the effective state at now;
  - `forge lease grant <root> <task-id> [--worker <id>] [--ttl-ms <ms>] --actor <actor>` (default
    TTL 15 minutes, bounded by `MAX_LEASE_DURATION_MS`);
  - `forge lease renew <root> <lease-id> [--ttl-ms <ms>] --actor <actor>`;
  - `forge lease release <root> <lease-id> --actor <actor>`;
  - `forge lease expire <root> --actor <actor>`.
- **Daemon:** `forged serve` runs a sweeper thread every 5 s. It takes the execution slot only if
  the slot is free, then calls `expire_leases(now, "forged")` and frees the slot. It skips the tick
  if the slot is busy or the lease lock is held. It stops with the daemon (a shared stop flag,
  joined before teardown). No protocol change.

## Invariants

- A task has at most one active lease, and a leased task never runs locally.
- Lease state changes only under the lease lock, and each change is audited.
- The daemon never appends to the audit log while an execution owns the slot.
- Registration and leases grant no execution authority.

## ADRs

ADR-0046 "Operator lease operations": profiles as registration, operator-as-worker until transport,
the launch guard, the lock, and the idle-only daemon sweep.

## Public API / CLI

The new `worker` and `lease` commands, the operator lease API, and `AuditEventKind::LeaseRecorded`.

## Compatibility analysis

- New commands and an additive audit kind. Older binaries reading a log with `LeaseRecorded`
  fail closed, as documented.
- The local launch guard changes behavior only for leased tasks, which cannot exist before this
  milestone.

## Dependency analysis

`agentforge-operator` gains the internal `agentforge-scheduler` dependency. It is verified by
`./scripts/package-preflight`. There are no external dependencies.

## Expected file boundary

- `.plans/P4-M004-lease-operations.plan.md`, `.plans/ACTIVE`
- `crates/agentforge-audit/src/lib.rs`, `crates/agentforge-audit/tests/*.rs`
- `crates/agentforge-operator/Cargo.toml`, `crates/agentforge-operator/src/lib.rs`,
  `crates/agentforge-operator/src/leases.rs`, `crates/agentforge-operator/tests/*.rs`
- `crates/agentforge-orchestrator/src/lib.rs`, `crates/agentforge-orchestrator/tests/*.rs`
- `crates/agentforge-daemon/src/lib.rs`, `crates/agentforge-daemon/tests/*.rs`
- `crates/agentforge-cli/src/main.rs`, `crates/agentforge-cli/tests/*.rs`
- `Cargo.lock`, `.cargo/registry-preflight.toml` (only if preflight needs the new edge)
- `docs/adr/ADR-0046-operator-lease-operations.md`, `docs/REMOTE_WORKERS.md`,
  `docs/OPERATIONS.md`, `docs/DAEMON.md`, `docs/DOGFOODING.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

- Audit: `LeaseRecorded` round-trips.
- Worker profiles: a valid profile loads; each of these fails closed: unknown key, bad value,
  symlink, oversize, `max_leases=0`, and no capability.
- Operator:
  - granting picks the first worker with capacity, and `--worker` pins one;
  - a second grant for the same task is refused;
  - a non-ready or non-pending task is refused;
  - a full worker makes the grant fail with nothing written;
  - renew extends the lease, and renewing an expired lease fails;
  - release frees the task;
  - expire marks due leases and audits each one;
  - each mutation appends exactly one audit event per lease change;
  - a held lock makes the mutation fail with nothing written.
- Launch guard: a leased task is refused before worktree creation by `launch_process_persisted`,
  and by `execute_process_persisted` with no state change. `launch-batch` skips it and runs the
  others. After release, the launch proceeds.
- Daemon: with a due lease, a running daemon expires it within the sweep interval and audits it.
  While an execution holds the slot, no sweep appends to the audit log.
- CLI: the full lifecycle through the real binary (grant, list, launch refused, renew, release,
  launch succeeds), and expire with a 1 ms TTL.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Audit kind, then the worker profiles and operator lease module, then the orchestrator guard,
   then the CLI, then the daemon sweeper, with tests at each step.
4. ADR, docs, and finding 9. Full gate plus package preflight. Dogfood on a scratch repo with
   `forged` running.
5. Push and verify CI (push plus a repeat); close; tag.

## Failure modes

- A stale lease lock after a crash blocks lease operations. The error names the file and says to
  remove it only when no `forge` or `forged` process holds it (the same policy as the daemon lock).
- A clock skew or jump could expire leases early. Lease windows are bounded, and the sweep only
  expires; it never grants.

## Documentation impact

REMOTE_WORKERS (profiles, commands, guard, sweep), DAEMON (sweeper), OPERATIONS (commands and
recovery), DOGFOODING (finding 9), and ADR-0046. README, CHANGELOG, and MILESTONES at closure.

## Quality gates

- `./scripts/gate.sh full` and `./scripts/package-preflight`;
- push-triggered CI green plus a dispatched repeat.

## Acceptance criteria

- [ ] Operators register workers and grant, renew, release, and expire leases from the CLI, with
      audit evidence.
- [ ] Leased tasks cannot run locally on any path.
- [ ] The daemon expires due leases without colliding with executions.
- [ ] ADR-0046, docs, and finding 9 recorded; CI evidence recorded; closed and tagged.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
