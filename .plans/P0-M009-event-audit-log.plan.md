# Plan: P0-M009 — Event and audit log

Status: Completed
Milestone: P0-M009
Created: 2026-09-19

## Goal

Add a standard-library-only append-only audit log that records orchestration evidence in a
versioned, deterministic, hash-chained format and fails closed on corruption or sequence loss.

## Non-goals

- No task-state transition engine, scheduler, policy decision, repair authority, notification,
  remote sink, database dependency, encryption, key management, or end-user audit CLI.
- Audit records are evidence and integrity signals; they do not grant approval or prove that a
  task outcome was correct.

## Context

P0-M004 supplies durable task-state revisions, P0-M005 supplies worktree identity, P0-M006 and
P0-M007 supply process evidence, and P0-M008 supplies exact-SHA CI evidence and classifications.
P0-M009 gives later orchestration code one durable local append boundary without changing those
existing contracts. The validated base main is `407f42214fb9099bc947dad3727b693a046cc04d`.

P0-M008 completion evidence is carried forward: implementation `6c2a057750e38fe1ec8c19c3618773283900b367`
with CI `35485116791`; closure `5b548bb53263c8e0dc10a5c3e9ad5edbffdad068` with CI
`35485184645`; merge `407f42214fb9099bc947dad3727b693a046cc04d` with post-merge CI
`35485226922`.

## Architecture placement

New `agentforge-audit` owns event validation, canonical encoding, append/replay, integrity-chain
verification, and bounded file I/O. It does not own task semantics, state snapshots, worktrees,
agents, gates, CI, permissions, or repair. Callers supply already-authorized facts and explicit
event payloads.

## Data flow

1. A caller constructs a versioned `AuditEvent` with an explicit sequence, event ID, kind, actor,
   optional task ID, injected wall-clock metadata, and bounded structured fields.
2. The log validates identifiers, field limits, sequence monotonicity, and event-specific required
   fields, then encodes one canonical length-delimited record.
3. Each record stores the previous record digest and its own digest. The file appends bytes and
   synchronizes them before reporting success.
4. Replay decodes records in order, verifies framing, canonical fields, sequence, and the complete
   integrity chain. Any partial tail, gap, duplicate, or digest mismatch is an explicit error.
5. Query helpers return deterministic sequence order and bounded filters; they never mutate or infer
   missing events.

## Invariants

- Records are append-only; no overwrite, truncation, reset, or silent repair is performed.
- Canonical encoding is stable across platforms and independent of map iteration order.
- Sequence numbers are contiguous and monotonic from an explicit genesis state.
- Event IDs are nonempty stable identifiers; actor and payload fields reject controls and enforce
  byte limits.
- Hash-chain verification detects reordered, deleted, duplicated, and modified records. The digest
  is an integrity signal, not a claim of cryptographic authenticity without external key custody.
- Replay is fail-closed and does not mutate the source file.
- Concurrent writers are rejected or serialized by the store boundary; tests must not rely on
  fixed shared paths.
- Audit evidence never grants authority, accepts a task, or initiates repair.

## Event model

The initial event kinds are `TaskCreated`, `ApprovalRecorded`, `WorktreeObserved`, `AgentStarted`,
`AgentFinished`, `GateFinished`, `CiObserved`, `FailureClassified`, `ReviewHandoff`, and
`TaskTransition`. Each carries an explicit version and bounded key/value payload. Unknown future
event kinds decode as an explicit compatibility error rather than being discarded.

## ADRs

ADR-0014 records the append-only audit boundary, canonical framing, and integrity-chain decision.
It follows ADR-0003, ADR-0004, ADR-0005, and ADR-0009.

## Public API / CLI

- `AuditEvent`, `AuditEventKind`, `AuditLog`, `AuditRecord`, `AuditStore`, and `AuditError`.
- `FileAuditStore` provides append and replay behind the trait boundary.
- No stable CLI, daemon integration, remote sink, or database backend in P0-M009.

## Encoding and integrity

The file format begins with a versioned magic header. Records use bounded little-endian lengths and
canonical UTF-8 fields, followed by a fixed-size digest chain. The implementation may include a
small self-contained SHA-256 routine or an equivalently documented deterministic digest; no
external Rust crate is introduced. Lengths are checked before allocation and trailing bytes are
rejected.

## Dependency analysis

Add one workspace crate with no external dependencies and no dependency changes to existing crates.

## Expected file boundary

Plan and closure checkpoints:

- `.plans/P0-M009-event-audit-log.plan.md`
- `.plans/ACTIVE` — created only after approval and removed at closure
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`
- `docs/MILESTONES.md`
- `docs/AUDIT.md`
- `docs/adr/ADR-0014-event-audit-log.md`
- `docs/adr/README.md`

Implementation checkpoint:

- `Cargo.toml`
- `Cargo.lock`
- `crates/agentforge-audit/Cargo.toml`
- `crates/agentforge-audit/src/**`
- `crates/agentforge-audit/tests/**`
- `docs/AUDIT.md`

No changes to core task semantics, state snapshots, worktrees, adapters, gates, CI monitor,
CLI/daemon, hooks, or workflow files without a plan amendment.

## Test-first matrix

| Behavior | Required evidence |
| --- | --- |
| Event validation | Required fields, IDs, versions, controls, payload limits, and unknown kinds fail closed |
| Canonical encoding | Equivalent events produce stable bytes and deterministic digests |
| Append/replay | Multiple records round-trip in sequence order with exact fields preserved |
| Integrity chain | Mutation, reorder, deletion, duplication, gap, wrong previous digest, and trailing bytes are detected |
| Crash safety | Partial header/record and oversized lengths return errors without mutation |
| Querying | Bounded task/kind/sequence filters preserve deterministic order |
| Durability boundary | Successful append synchronizes before return; I/O failures remain explicit |
| Concurrency/isolation | Unique temporary files, serialized writers, and no parent environment mutation |
| Compatibility | Version mismatch and unknown event kind are explicit errors |

## Implementation sequence

1. Review this Draft plan and ADR-0014; obtain approval before activating implementation.
2. Commit `Status: Approved`, active pointer, and activation documents separately from code; require
   exact plan CI green.
3. Add event types, validation, canonical framing, digest chain, and in-memory tests.
4. Add file store append/replay, synchronization, corruption detection, and isolated integration
   tests.
5. Document the format, durability, integrity limits, and non-authority semantics.
6. Run the full local gate and Rust 1.85.0 focused tests; inspect the declared boundary.
7. Commit implementation and validate exact CI, then close, merge, and validate post-merge main.

## Failure modes

- Treating a valid digest as proof of authorization or truth rather than tamper evidence.
- Silently truncating a partial tail, rewriting history, skipping a sequence, or accepting unknown
  event kinds.
- Unbounded allocation from file lengths, nondeterministic map serialization, or timestamp-based
  ordering used as the sole sequence authority.
- Allowing concurrent writers to interleave records or leaking audit payloads into shell commands.

## Documentation impact

Document event kinds, canonical format, digest-chain semantics, durability behavior, query limits,
and security boundaries in `docs/AUDIT.md`.

## Quality gates

Run `./scripts/gate.sh full` and focused audit tests on stable and Rust 1.85.0. Require Repository
policy, Stable code gate, MSRV 1.85.0, and CLI smoke green for exact approved-plan,
implementation, closure, and post-merge SHAs.

## Acceptance criteria

- [x] Approved plan is committed and green before implementation.
- [x] Versioned append/replay and canonical integrity chain are implemented and tested.
- [x] Corruption, partial writes, gaps, duplicates, unknown versions, and oversized input fail closed.
- [x] Deterministic bounded queries and serialized durable appends are tested.
- [x] No external dependency, authority grant, automatic repair, or unrelated subsystem change is introduced.
- [ ] Exact implementation, closure, and post-merge CI are green.

## Completion record

Implementation commit: 30b557369f3298dceb706935993fcb358ce62c0c
Implementation CI: 35489676966 — all four jobs green
Closure commit: pending
Closure CI: pending
Post-merge main: pending
Post-merge CI: pending
Completed: pending

Evidence:

- Draft plan checkpoint: `b6c8cf1bf41ad3bb14ea6de0a02f15f2ee045d19`;
  CI run `35485381741` was green.
- Approved plan checkpoint: `67a0b2abe8794e348277b1ea41a362fc578d8a2a`;
  CI run `35485474803` was green.
- The full local gate and focused audit tests passed for the implementation.
- Exact implementation CI run `35489676966` passed Repository policy, Stable code gate,
  MSRV 1.85.0, and CLI smoke for `30b557369f3298dceb706935993fcb358ce62c0c`.
