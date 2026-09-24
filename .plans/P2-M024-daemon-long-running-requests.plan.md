# Plan: P2-M024 — Daemon requests that outlive the client read timeout

Status: Approved
Milestone: P2-M024
Created: 2026-09-23
Owner: AgentForge project

## Goal

Make `forge daemon run` and `forge daemon launch` work for agents that run longer than two seconds.
Keep the daemon answering `status` while an execution is in progress, and give `stop` a defined,
safe behavior during an execution. Never tell an operator to delete metadata that a live daemon
owns.

## Non-goals

- No concurrent task execution in the daemon. One execution at a time stays the rule; batch launch
  stays in the foreground CLI (P1-M006).
- No change to the endpoint file, the loopback-only bind, or request/response size limits.
- No cancellation of a running agent through the daemon.

## Context

Reproduced on 2026-09-23 with a three-second agent profile. The client read timeout is 2 seconds
for every request, but `Run`, `RunProfile`, `Launch`, and `LaunchProfile` execute the agent (and,
since P1-M004, its gates) before responding. After 2 seconds the client maps the timeout to
`StaleInstance` and prints "stale daemon metadata ... remove it only after confirming no daemon owns
the project" while the live daemon is still running the task. The accept loop is single-threaded,
so `status` from another terminal during the run fails the same way, and a `stop` sent during a run
is only processed after the run ends.

Classification: semantic/test. Every daemon test used agents that finish in well under 2 seconds.

## Architecture placement

`agentforge-daemon` only.

- Server: the accept loop reads each request frame (bounded, as in P2-M023). `Status` and `Stop`
  are answered on the accept loop. Execution requests are handed to a worker thread, which owns
  the connection until it responds. A shared execution slot allows one execution at a time.
- Protocol: while executing, the worker sends an `AFD1\tOK\tPENDING` keepalive frame every second,
  then the final response.
- Client: execution requests read frames until the final response. Each read is bounded by a
  10-second idle timeout, so liveness stays bounded without guessing the agent's runtime.

## Data flow

1. `status` is always answered immediately, even during an execution.
2. An execution request while the slot is free takes the slot and runs on a worker thread,
   streaming keepalives, then the final response, then releases the slot.
3. An execution request while the slot is taken gets an immediate error naming the running task.
4. `stop` while the slot is taken gets an immediate error: "daemon is executing <task>; stop after
   it finishes". Stopping never abandons a running agent or its state writes.
5. If the connection to the daemon drops or goes idle during an execution request, the client
   reports that the daemon stopped responding and that the task's state should be inspected. It
   never suggests removing metadata.

## Invariants

- At most one task execution at a time per daemon.
- `stop` never exits the daemon while an execution holds the slot.
- Every client read is bounded: 2 seconds for control requests, 10 seconds of idle time for
  execution requests.
- The accept loop never blocks on an agent.

## ADRs

- ADR-0040: long-running daemon requests use keepalive frames and a single execution slot.

## Public API / CLI

- New `DaemonError::Busy(String)` and `DaemonError::ExecutionInterrupted(String)`.
- New `PENDING` keepalive response frame inside the existing `AFD1` protocol. `forge` and `forged`
  ship together from one build, so the endpoint version stays `1`.
- CLI behavior is unchanged apart from long runs now succeeding and the clearer busy and
  interrupted messages.

## Compatibility analysis

A `forged` from this change sends `PENDING` frames that an older `forge` would reject as malformed.
The two binaries are always released together (P2-M004), and this is documented. Short executions
behave as before.

## Dependency analysis

No new dependency. Standard-library threads, channels, and a mutex.

## Expected file boundary

- `.plans/P2-M024-daemon-long-running-requests.plan.md`
- `.plans/ACTIVE`
- `crates/agentforge-daemon/src/lib.rs`
- `crates/agentforge-daemon/tests/daemon.rs`
- `crates/agentforge-cli/src/bin/agentforge-cli-fixture.rs` (sleep mode)
- `crates/agentforge-cli/tests/daemon_commands.rs`
- `docs/DAEMON.md`
- `docs/adr/ADR-0040-daemon-long-running-requests.md`
- `README.md`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

## Test-first matrix

- A daemon launch whose agent sleeps 3 seconds succeeds. This reproduces the bug first.
- `status` answers while that execution is in progress.
- A second execution request during it gets `Busy` and records no task changes.
- `stop` during it is refused; `stop` after it completes succeeds.
- Keepalive and final frames parse; an unknown frame is still rejected.
- The existing CLI daemon test's fixed 500 ms readiness poll and unbounded join are replaced with
  deadlines.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Add the sleep fixture mode and the failing long-execution CLI test.
4. Add the execution slot, worker thread, keepalive frames, and client frame loop.
5. Add the busy, stop-during-execution, and status-during-execution tests.
6. Update docs, README, and ADR-0040.
7. Run the full gate, push to `cybercore-tech/agentforge`, and record repeated exact-SHA CI
   evidence before closure.

## Failure modes

- A worker thread panic releases the slot (guard drop) and closes the connection; the client
  reports `ExecutionInterrupted`.
- A client that disconnects mid-execution does not stop the execution; the worker ignores write
  errors and still persists task state.

## Documentation impact

`docs/DAEMON.md` documents keepalives, the execution slot, busy and stop semantics, and the new
errors. README notes that long agents work through the daemon. ADR-0040 records the decision.

## Quality gates

- `./scripts/gate.sh full`;
- exact-SHA CI green on all seven jobs across repeated runs.

## Acceptance criteria

- [ ] Daemon executions longer than the control timeout succeed.
- [ ] `status` answers during an execution; overlapping executions and `stop` are refused.
- [ ] No client error suggests removing metadata owned by a live daemon.
- [ ] Full local validation and exact-SHA CI evidence are recorded before closure.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
