# Plan: P2-M023 — Bounded daemon lifecycle tests and CI job timeouts

Status: Approved
Milestone: P2-M023
Created: 2026-09-23
Owner: AgentForge project

## Goal

Eliminate the recurring Windows CI hang in the foreground daemon lifecycle tests, make every
daemon lifecycle test fail with bounded diagnostics instead of blocking forever, stop a stalled
client from wedging the single-threaded daemon, and give every CI job an explicit timeout so a
future hang cannot hold a runner for six hours.

## Non-goals

- No daemon protocol, endpoint format, or CLI change.
- No change to the task run/launch request semantics or their client timeouts.
- No weakening of lifecycle assertions; every start/status/stop/restart check is retained.
- No reduction of platform coverage; Windows keeps running the daemon suite.

## Context

AgentForge CI run `35697199910` for `69bc572` was cancelled after six hours because the Windows
platform job never finished. The same signature appears in runs `35565106069`, `35564310579`, and
`35555944287`: `daemon_lifecycle_is_loopback_only_and_cooperative` runs past 60 seconds and the
other lifecycle test waits behind it. P2-M021 attributed this to concurrent lifecycle tests and
serialized them with a mutex; the hang recurred after that change, so that diagnosis was
incomplete.

Root cause (semantic/test classification): the foreground tests poll readiness for a fixed
50 × 10 ms (500 ms) budget. `serve` canonicalizes the root, runs `git rev-parse` through
`WorktreeManager::new`, and fsyncs lock and endpoint files before publishing its endpoint. On a
slow Windows runner this can exceed 500 ms. The test then treats the daemon as failed and calls
`server.join()` on a server that started successfully and is now waiting for requests, so the
join never returns.

The hang reproduces locally by placing a `git` wrapper that sleeps one second ahead of the real
`git` on `PATH`: the test is still blocked when `timeout 90` terminates it.

A separate daemon robustness gap has the same effect: accepted connections have no read timeout,
so one client that connects and sends nothing blocks every later request, including `stop`.

## Architecture placement

- `agentforge-daemon` test harness: deadline-based readiness, bounded thread joins.
- `agentforge-daemon` server loop: bounded per-connection request reads/writes.
- `.github/workflows/ci.yml`: per-job `timeout-minutes`.

## Data flow

1. The test spawns the foreground server thread, which reports its result over a channel.
2. The test polls `status` until an explicit deadline, or until the server thread reports an
   early exit.
3. Teardown sends `stop`, then waits for the server result with a bounded receive; a timeout
   fails the test with diagnostics instead of blocking.
4. The server applies a bounded read/write timeout to each accepted connection before reading its
   request frame; a timed-out connection receives a best-effort error and the loop continues.

## Invariants

- No test can block without bound on a daemon thread or process.
- Every existing lifecycle assertion still runs on Linux, macOS, and Windows.
- A client that connects and never sends a frame cannot prevent a later `stop` from succeeding.
- Protocol, endpoint, lock, and stale-instance semantics are unchanged.

## ADRs

None. This is a reliability repair within existing daemon decisions.

## Public API / CLI

None.

## Compatibility analysis

The server-side request timeout only affects clients that stall mid-frame for longer than the
bound; every existing client writes one complete frame immediately after connecting. The existing
client-side connect/read/write timeouts are unchanged.

## Dependency analysis

No new dependency; standard library only (`std::sync::mpsc`, `Instant`).

## Expected file boundary

- `.plans/P2-M023-bounded-daemon-lifecycle-tests.plan.md`
- `.plans/ACTIVE`
- `crates/agentforge-daemon/src/lib.rs`
- `crates/agentforge-daemon/tests/daemon.rs`
- `.github/workflows/ci.yml`
- `crates/agentforge-cli/tests/task_launch_commands.rs` (amendment 1)
- `crates/agentforge-orchestrator/tests/vertical_slice.rs` (amendment 1)
- `crates/agentforge-intake/src/lib.rs` (test helper only; amendment 1)
- `docs/CI.md`
- `docs/DAEMON.md`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

## Amendment 1 — repeat-run evidence

Implementation `1eb3b0f` passed exact CI `35960567335` on all seven jobs, and Windows passed in
three further dispatched runs on the same SHA. Two of those runs exposed two older intermittent
failures on other hosts:

- Run `35960725620` (MSRV, Linux): `spawned_daemon_start_and_restart_are_bounded_and_cooperative`
  failed with `stale daemon metadata ... .forge/daemon/lock`. Classification: semantic/test, a
  daemon teardown race. `Server::drop` removes the endpoint before the lock, and `stop` treats a
  missing endpoint as complete teardown, so a restart can start a new `forged` while the previous
  lock file still exists. Repair: `stop` waits until both the endpoint and the lock are gone,
  within the existing stop bound. The drop order stays the same, so a new daemon can never have
  its endpoint removed by the previous daemon's teardown.
- Run `35960716878` (macOS): `task_launch_prepares_and_runs_one_real_foreground_task` failed with
  `AlreadyExists` creating its temporary root. Classification: semantic/test, a fixture collision.
  The helper names roots only by a nanosecond timestamp, and macOS timestamps have microsecond
  resolution, so parallel tests in one binary can collide. Repair: add the process ID and a
  per-process counter to every multi-test fixture helper with the same pattern (task launch CLI
  tests, orchestrator vertical-slice tests, intake unit tests).

Closure additionally requires repeated dispatched runs on the repair SHA with no failures.

## Amendment 2 — Windows delete-pending teardown

Repair `4f5a45e` passed its push run and three of four dispatched runs. Run `35961023320`
(Windows) failed `stalled_client_does_not_block_stop` because `stop` returned
`Io(PermissionDenied, "Access is denied.")`. Classification: semantic/test. On Windows a file whose
deletion is still pending reports `Access is denied` instead of `NotFound`, so `stop` read the
endpoint mid-removal and returned an error instead of continuing to observe teardown. This is also
the most likely source of the historical Windows teardown `PermissionDenied` failures recorded
since P2-M020. `Path::exists` has the same problem for the lock file: it reports a delete-pending
lock as absent even though it still blocks re-creation. Repair: while waiting for a stop, treat
`PermissionDenied` on the endpoint as "still tearing down", and count the lock as removed only on
`NotFound`, all within the existing bound. No file-boundary change.

## Test-first matrix

- With a `git` that sleeps one second on `PATH`, the foreground lifecycle and disconnected-client
  tests complete instead of hanging.
- A client that connects and sends nothing does not stop a later cooperative `stop` from
  succeeding.
- A server that fails to start reports its error within the readiness deadline.
- Existing lifecycle, stale-identity, malformed-endpoint, and spawned restart tests stay green.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Add a foreground-daemon test helper with deadline readiness and bounded joins; port the
   foreground tests to it.
4. Add the stalled-client regression test and the server per-connection timeout.
5. Add `timeout-minutes` to every CI job and document the bound.
6. Run the full local gate plus the slow-`git` reproduction.
7. Push and record exact-SHA CI evidence, then close in a separate checkpoint.

## Failure modes

- If the Windows job still exceeds its timeout, the job fails quickly with the test name in the
  log; classify it before any further repair.
- A readiness deadline that is exceeded fails with the server thread's reported result.

## Documentation impact

`docs/CI.md` records per-job timeouts. `docs/DAEMON.md` records the per-connection request bound.

## Quality gates

- `./scripts/gate.sh full`;
- the slow-`git` reproduction passes;
- exact-SHA CI green on all seven jobs, including Windows.

## Acceptance criteria

- [ ] The Windows hang's root cause is identified, reproduced, and classified.
- [ ] Daemon lifecycle tests cannot block without bound.
- [ ] A stalled client cannot wedge the daemon.
- [ ] Every CI job has an explicit timeout.
- [ ] Full local validation and exact-SHA matrix evidence are recorded before closure.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
