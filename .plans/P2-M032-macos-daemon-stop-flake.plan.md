# Plan: P2-M032 — macOS daemon stop flake (EINVAL on a reset socket)

Status: Draft
Milestone: P2-M032
Created: 2026-09-24
Owner: AgentForge project

## Goal

Make `forge daemon stop` reliable on macOS while the daemon is closing its listener, so the
intermittent `daemon I/O failed: Invalid argument (os error 22)` no longer fails stops or CI.

## Non-goals

- No change to the daemon protocol, execution slot, or stop semantics.
- No blanket retry of control requests.

## Context

CI run `36005350819` (macOS 14) failed the P2-M024 test
`long_daemon_executions_succeed_while_status_stays_available`: its final `forge daemon stop`
returned `daemon I/O failed: Invalid argument (os error 22)`. The same commit passed that job in
three other runs.

Analysis: after the daemon acknowledges `Stop`, `stop()` polls `status` until the endpoint file
disappears. A poll can connect into the listen backlog just as the daemon drops its listener, which
resets the queued connection. Linux accepts the client's following `setsockopt` (read/write timeout)
on that reset socket. macOS rejects it with `EINVAL`, which Rust reports as
`io::ErrorKind::InvalidInput`. The client already treats `ConnectionReset`, `ConnectionAborted`, and
`NotConnected` as a stale endpoint (keep polling while stopping). `InvalidInput` from the same
situation was not in that list, so the stop failed instead of continuing to poll.

Classification: semantic/test (platform-specific error mapping in the daemon client).

## Architecture placement

`agentforge-daemon::map_transport_error`, which is used only for errors from the client's own socket
calls (connect, set timeouts, write, flush, read). It now also classifies `InvalidInput` as a
transport loss (`StaleInstance`). Every timeout the client passes is a nonzero constant, so
`InvalidInput` there can only come from the platform rejecting an operation on a reset socket.
Execution requests already map post-write failures to `ExecutionInterrupted` and are unchanged.

## Invariants

- `stop` still fails with a real error (for example `PermissionDenied`, or `BUSY`) when one occurs.
- Only socket-call errors from the client are affected.

## ADRs

None.

## Public API / CLI

None.

## Compatibility analysis

None; stops that used to fail spuriously on macOS now complete.

## Dependency analysis

None.

## Expected file boundary

- `.plans/P2-M032-macos-daemon-stop-flake.plan.md`
- `.plans/ACTIVE`
- `crates/agentforge-daemon/src/lib.rs`
- `docs/DAEMON.md`
- `CHANGELOG.md`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

## Test-first matrix

- A unit test maps `InvalidInput`, `ConnectionReset`, `WouldBlock`, and `TimedOut` to
  `StaleInstance`, and keeps `PermissionDenied` as an `Io` error.
- Existing daemon lifecycle and long-execution tests pass.
- CI: the push run plus at least three dispatched repeats are green, including macOS.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Fix the mapping, add the unit test, and document it.
4. Push, run repeat CI, close, and tag.

## Failure modes

- If macOS still fails the stop after the fix, classify the new evidence before any further change.

## Documentation impact

`docs/DAEMON.md` platform note; CHANGELOG Fixed entry.

## Quality gates

- `./scripts/gate.sh full`;
- the push run plus at least three repeat CI runs green on all seven jobs.

## Acceptance criteria

- [ ] `InvalidInput` from client socket calls is treated as transport loss.
- [ ] Repeated CI is green on macOS.
- [ ] Documented, closed, and tagged.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
