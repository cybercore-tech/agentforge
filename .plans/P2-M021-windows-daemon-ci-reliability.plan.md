# Plan: P2-M021 — Windows daemon CI reliability

Status: Draft
Milestone: P2-M021
Created: 2026-09-20
Owner: AgentForge project

## Goal

Make the spawned daemon start/restart/stop lifecycle deterministic on the supported Windows CI
runner so the exact-head cross-platform matrix is a reliable release signal.

## Context

The P2-M020 closure matrix reproduced an existing Windows-only failure in
`spawned_daemon_start_and_restart_are_bounded_and_cooperative`: after a cooperative restart, the
final `stop` can fail with `io::ErrorKind::PermissionDenied` (`Access is denied.`). The same test
passes repeatedly on Linux, and the fixture already allocates a unique temporary repository for
each test, so the current evidence points to process/socket/file teardown timing rather than a
shared fixture path. The daemon currently waits for endpoint disappearance, retries transient
executable launch denial, and maps common transport failures, but does not make every Windows
teardown observation explicit or deterministic.

## Scope

- Trace the spawned daemon lifecycle across endpoint publication, cooperative stop response,
  server drop cleanup, child exit, and the subsequent restart/stop sequence.
- Harden the daemon's bounded lifecycle observation and cleanup handling for transient Windows
  process, endpoint, lock-file, and loopback transport timing while preserving fail-closed stale
  instance behavior and bounded timeouts.
- Strengthen the daemon integration harness so each spawned child is deterministically observed and
  reaped, temporary roots remain unique, and cleanup failures retain actionable diagnostics.
- Add focused regression coverage for the restart/stop sequence and any Windows-specific cleanup
  condition discovered during implementation; keep the same lifecycle assertions active on all
  platforms.
- Record the root cause, classification, and exact implementation/CI evidence in milestone state
  and handoff documentation.

## Non-goals

- No weakening, deletion, quarantine, retry-only masking, or platform exclusion of the failing
  lifecycle test.
- No unbounded sleeps, forced process termination as normal cleanup, forced worktree cleanup, or
  suppression of `PermissionDenied` errors without a verified ownership/teardown condition.
- No change to daemon protocol semantics, task execution authority, capabilities, audit format, or
  unrelated CLI behavior.
- No changes to the supported OS matrix or CI pass criteria; workflow edits are allowed only for
  bounded diagnostics that make a real lifecycle failure explainable.

## Architecture placement

The change remains inside the daemon lifecycle boundary: `agentforge-daemon` owns the spawned
`forged` child, endpoint/lock metadata, cooperative stop protocol, bounded readiness observation,
and test-only process cleanup. Worktree, task, audit, and adapter contracts remain unchanged.

## Invariants

- A daemon is considered stopped only after its endpoint is absent and its owned child has exited
  or the bounded lifecycle operation returns an explicit error.
- A stale endpoint/lock is never silently adopted or deleted by a normal start/stop operation.
- All process operations remain direct argument-array invocations with bounded waits.
- Windows-specific handling must be narrowly conditioned on an observed transient lifecycle state;
  unrelated I/O errors remain visible to the operator and CI.
- The test harness must not share project roots or daemon metadata between concurrent tests.

## Expected file boundary

- `.plans/P2-M021-windows-daemon-ci-reliability.plan.md`
- `.plans/ACTIVE`
- `crates/agentforge-daemon/src/lib.rs`
- `crates/agentforge-daemon/tests/daemon.rs`
- `.github/workflows/ci.yml` (only if bounded diagnostic output is required)
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

## Test-first matrix

- Repeat the focused spawned lifecycle test serially and with the workspace's normal test
  parallelism on the current host.
- Exercise daemon start, cooperative stop, restart, final stop, endpoint disappearance, and child
  reaping as one bounded scenario.
- Preserve malformed-endpoint, stale-identity, disconnected-client, and foreground lifecycle tests.
- Run the full local gate on the exact implementation head.
- Verify the exact implementation and closure SHAs on Ubuntu, macOS, and Windows CI; a Windows
  failure must be classified and repaired before closure.

## Implementation sequence

1. Reproduce and instrument the focused lifecycle path enough to identify whether the denial occurs
   during endpoint read/connect, server metadata removal, child wait, or temporary-root cleanup.
2. Implement the smallest daemon lifecycle correction that preserves bounded, cooperative semantics
   and explicit errors.
3. Update regression tests/fixtures to assert deterministic ownership and cleanup without hiding
   failures.
4. Run formatting, focused tests, the full gate, and exact-head CI.
5. Record root cause and evidence in this plan, `docs/MILESTONES.md`, `PROJECT_STATE.md`, and
   `AGENT_HANDOFF.md`, then perform the separate closure checkpoint.

## Failure modes

- If the denial is a runner-level transient after the daemon has demonstrably exited, retain the
  evidence and add a bounded, ownership-checked observation path rather than a blanket retry.
- If the denial exposes a real endpoint/lock cleanup race, repair the server/drop or client wait
  ordering and add a regression assertion.
- If a failure is unrelated infrastructure, classify it separately and do not broaden this plan.

## Quality gates

- `cargo fmt --all --check`
- `cargo test -p agentforge-daemon --test daemon --locked`
- `CARGO_TARGET_DIR=/tmp/agentforge-cargo-target ./scripts/gate.sh full`
- Exact-SHA GitHub CI green across repository policy, stable, MSRV, CLI smoke, Ubuntu, macOS, and
  Windows.

## Acceptance criteria

- [ ] The exact Windows teardown race is identified with evidence and classified.
- [ ] Spawned daemon start/restart/stop is bounded, cooperative, and deterministic on Windows.
- [ ] Regression coverage remains active on every supported platform and passes without masking.
- [ ] Existing protocol, stale-instance, and foreground lifecycle behavior remains green.
- [ ] Full local validation and exact-SHA matrix evidence are recorded before closure.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
