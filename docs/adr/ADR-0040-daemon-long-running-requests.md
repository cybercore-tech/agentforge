# ADR-0040: Keepalive frames and a single execution slot for daemon executions

- Status: Accepted
- Date: 2026-09-23
- Milestone: P2-M024

## Context

Every daemon request used the same 2-second client read timeout, but `run` and `launch` execute the
agent (and its gates) before responding. Any agent that ran longer than 2 seconds made the client
report "stale daemon metadata ... remove it" while the daemon was still running the task, which is
wrong and would be harmful advice to follow. The accept loop was single-threaded, so `status`
failed the same way during a run, and `stop` had no defined behavior mid-run.

## Decision

- The accept loop answers `status` and `stop` itself and hands each execution to a worker thread.
- A single execution slot, taken only on the accept-loop thread, allows one execution at a time. A
  second execution, or a `stop`, during a run gets a `BUSY` response and changes nothing.
- The worker streams an `AFD1 OK PENDING` keepalive every second until the final response. The
  client bounds idle time (10 seconds) instead of total time; the agent profile bounds the run.
- A lost connection after an execution was accepted is reported as `ExecutionInterrupted`, which
  points to task inspection, never to removing metadata.

## Consequences

Positive:

- daemon runs work for real agents of any length within their profile timeout;
- status stays available and stop can never abandon an in-progress execution;
- the misleading "remove stale metadata" advice can no longer appear for a live daemon.

Trade-offs:

- the daemon still runs one task at a time (concurrent batches remain in the foreground CLI);
- `forge` and `forged` must come from the same build because of the new frames;
- a client that disconnects cannot cancel the execution it started.
