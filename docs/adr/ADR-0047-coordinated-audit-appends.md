# ADR-0047: Coordinated audit appends

- Status: Accepted
- Date: 2026-09-24
- Milestone: P0-M013
- Amends: ADR-0009-era audit store behavior (P0-M009)

## Context

`FileAuditStore` read the log once at `open` and kept the chain tail (next sequence, previous
digest) in memory. `append` linked each new frame to that in-memory tail. Any second writer made
the tail stale:
- a daemon execution holds its handle for minutes;
- the CLI can approve or lease meanwhile;
- the coming worker process will write constantly.

The stale writer's next frame reused a sequence number and linked to an old digest, and the next
`open` failed the integrity check. That is a corrupted log, not a refused write (dogfooding
finding 9, reproduced in `concurrent_appends.rs`). A threaded test also showed that `open`
exposed a partial header while creating a new log.

Separately, `forge init` and `forge task create` did not create the log, and the operator crate
refused to create it, so a fresh project could not record its first approval (finding 10).

## Decision

- **Append lock.** Every append takes `<log>.lock` (create-new, removed on drop, a 5 s wait, never
  stolen). It is held only to refresh, write one buffer, and fsync.
- **Refresh.** Under the lock, the store reads the bytes appended since its verified length, and
  verifies them as a continuation of its chain, into a copy. A shrunken file or a broken
  continuation fails closed and leaves the handle unchanged.
- **Renumber.** An event numbered for this handle's pre-refresh view gets the next real sequence,
  and a trailing `-<sequence>` in its ID moves with it. Other sequences are still refused, so
  caller bugs are not hidden. Callers keep numbering from their own handle and need no retries.
- **Batches.** `append_batch` writes a contiguous attempt log under one lock, renumbered as a
  block, so an execution's records are never interleaved. The orchestrator persists every attempt
  log this way.
- **Atomic creation.** A new log's header is written to a private file and hard-linked into place,
  so readers never see a partial header.
- **First use.** Operator actions create `.forge/audit.log` when it is missing, but only when the
  task snapshot exists, so a log is never created outside an initialized project.
- **Readers stay lock-free.** Each frame is written in one call. HUD and inspection reads never
  block writers.

## Consequences

Positive:

- any number of `forge` processes, the daemon, and future workers can append safely;
- the daemon sweep's use of the execution slot (ADR-0046) is no longer needed for integrity. It is
  kept because it also keeps sweeps out of executions' way.

Negative:

- a crash inside the millisecond write window leaves a stale `audit.log.lock` that blocks appends
  until an operator removes it (the same policy as the daemon and lease locks);
- event IDs with a sequence suffix can change on renumbering. The sequence is the authoritative
  position; the ID is a label;
- binaries from before this change do not take the lock, so mixing old and new writers stays
  unsafe.

## Alternatives considered

- **OS advisory locks (`File::lock`).** Stabilized after the 1.85 MSRV, and platform crates would
  add dependencies.
- **Reject stale appends and make every caller retry.** That would touch every call site and
  still leave attempt logs vulnerable to interleaving.
