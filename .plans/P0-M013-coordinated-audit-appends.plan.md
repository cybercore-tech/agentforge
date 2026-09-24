# Plan: P0-M013 — Coordinated audit appends and first-use audit logs

Status: Draft
Milestone: P0-M013
Created: 2026-09-24
Owner: AgentForge project
Implementer: operator-authored (audit integrity is a governance boundary)

## Goal

Make the audit log safe with more than one writer, and usable from a project's first command:

1. **Finding 9.** `FileAuditStore` appends are coordinated across handles and processes. A writer
   whose view is stale never corrupts the chain. Its events are placed after whatever other
   writers appended.
2. **Finding 10.** Operator commands (`task approve`, `accept`, `cancel`, `retry`, `integrate`,
   `ci observe`, and leases) create `.forge/audit.log` on first use in a real project, instead of
   failing with "audit log is missing".

## Non-goals

- No audit format change and no migration.
- No read locking: readers stay lock-free.
- No change to what events are recorded.

## Context

`FileAuditStore` keeps the chain tail (next sequence, previous digest) in memory from `open`.
`append` writes a frame linked to that in-memory tail. If another handle appended in the meantime
(a daemon execution holds its handle for minutes; the CLI can run `task approve` meanwhile), the
new frame reuses a sequence number and links to a stale digest. The next `open` fails with an
integrity mismatch. **The log is corrupted, not merely refused.** That is worse than finding 9
described. P4-M004 kept the daemon's own sweep out of the way by sharing the execution slot, but
the cross-process case remains open.

Finding 10 comes from the P4-M004 CLI test. `forge init` and `forge task create` do not create the
audit log, and the operator crate's `open_existing_audit` refuses to create it. A task with a
pre-execution approval therefore cannot be approved on a fresh project.

The same-host worker process (P4-M005, next) adds a long-lived writer running alongside the CLI
and daemon, so this must land first.

## Architecture placement

- **`agentforge-audit` — append lock:** `<log>.lock` (for example `.forge/audit.log.lock`),
  create-new, removed on drop. A writer waits up to 5 s, and the lock is never stolen. It is held
  only for refresh, write, and fsync (milliseconds). A lock left by a crash produces an error that
  names the file and says to remove it when no `forge` or `forged` process is running for the
  project, the same policy as the daemon and lease locks.
- **Refresh under the lock:** the store tracks its verified byte length. It reads only the bytes
  appended since, and decodes and verifies them as a continuation of its chain (same checks as
  `open`). A shorter file, or bytes that do not continue the chain, fail closed.
- **Renumbering:**
  - `append(event)` accepts `event.sequence` equal to the fresh next sequence. It also accepts the
    sequence this handle expected before the refresh, in which case the event is renumbered to the
    fresh next sequence. Any other sequence is still an error.
  - A trailing `-<old sequence>` in the event ID is renumbered with it, which keeps the ID
    convention (`lease-granted-12`).
- **`append_batch(events)`:** appends several pre-numbered, contiguous events (an attempt log)
  under one lock. It renumbers them by one shared offset, and no other writer can interleave.
  `persist_execution` in the orchestrator uses it.
- **Writes:** each frame (length plus payload) is written with one `write_all` of a single buffer,
  then synced.
- **`agentforge-operator`:** `open_existing_audit` becomes `open_project_audit`. It creates
  `.forge/` and the log when they are missing, but only after the caller has loaded the task
  snapshot, which proves the root is a project. The lease module drops its own creation code and
  uses it.

## Invariants

- Every record's previous-digest link and sequence continue the verified on-disk tail, whatever
  other writers did.
- Callers keep computing sequences from their own handle; they need no retry logic.
- The audit log is never created outside an initialized project.

## ADRs

ADR-0047 "Coordinated audit appends": lock, refresh, renumber, batch, and why readers stay
lock-free.

## Public API / CLI

- `FileAuditStore::append` behavior (refresh and renumber), and the new
  `FileAuditStore::append_batch`.
- CLI behavior: operator commands work on a fresh project.

## Compatibility analysis

- The file format is unchanged. Older binaries can read logs written by this version. Older
  binaries writing alongside newer ones do not take the lock, and they remain unsafe, as before.
- An append that used to corrupt the log now succeeds with a renumbered sequence.

## Dependency analysis

None.

## Expected file boundary

- `.plans/P0-M013-coordinated-audit-appends.plan.md`, `.plans/ACTIVE`
- `crates/agentforge-audit/src/lib.rs`, `crates/agentforge-audit/tests/*.rs`
- `crates/agentforge-orchestrator/src/lib.rs`
- `crates/agentforge-operator/src/lib.rs`, `crates/agentforge-operator/src/leases.rs`,
  `crates/agentforge-operator/tests/*.rs`
- `crates/agentforge-cli/tests/*.rs`
- `docs/adr/ADR-0047-coordinated-audit-appends.md`, `docs/DOGFOODING.md`, `docs/OPERATIONS.md`,
  `docs/DAEMON.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

- **Reproduction first:** two handles open the same log, and both append using their own view.
  Before the fix, the second append corrupts the log (`open` fails). After it, both records
  survive, the second is renumbered (including its event-ID suffix), and `open` verifies the
  chain.
- A stale handle appending several events in sequence stays contiguous.
- `append_batch` from a stale attempt log is renumbered as a block. A non-contiguous batch is
  refused, with nothing written.
- A sequence that is neither fresh nor this handle's expected sequence is still refused.
- A held lock gives a clear error after the wait, with nothing written, and succeeds once
  released.
- A log truncated underneath a handle fails closed on the next append.
- **Stress:** 4 threads × 25 appends, each thread with its own handle; the result is 100 records
  and a verified chain.
- **Cross-process:** two `forge` processes approving and leasing in a loop against one project;
  the log stays valid (CLI test).
- **Finding 10:** `forge init` → `task create --approval ...` → `task approve` succeeds on a fresh
  project and creates `.forge/audit.log`. The operator API does not create a log without a task
  snapshot.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Write the reproduction test and see it fail. Then lock, refresh, renumber, and batch; then the
   orchestrator's `persist_execution`; then the operator `open_project_audit`.
4. ADR, docs, and findings 9 and 10 resolved. Full gate plus package preflight.
5. Push and verify CI (push plus two repeats, because the change is concurrency-related); close;
   tag.

## Failure modes

- A stale append lock after a crash inside the millisecond write window: the error names the
  file, with the documented removal rule.
- Contention over 5 s (it should not happen, since holds are milliseconds): a clear error, and the
  caller can retry.

## Documentation impact

ADR-0047, OPERATIONS (lock recovery row), DAEMON (the sweep note: the slot is no longer needed for
safety but is kept), and DOGFOODING (findings 9 and 10 resolved). README, CHANGELOG, and
MILESTONES at closure.

## Quality gates

- `./scripts/gate.sh full` and `./scripts/package-preflight`;
- push-triggered CI plus two dispatched repeats.

## Acceptance criteria

- [ ] Concurrent writers can no longer corrupt the audit log; proven by reproduction, stress, and
      cross-process tests.
- [ ] Operator commands work on a fresh project.
- [ ] ADR-0047 and docs; findings 9 and 10 resolved; CI evidence; closed and tagged.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
