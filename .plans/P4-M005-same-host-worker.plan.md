# Plan: P4-M005 — Same-host worker process

Status: Draft
Milestone: P4-M005
Created: 2026-09-24
Owner: AgentForge project
Implementer: operator-authored (lease-holder execution is an authority boundary)

## Goal

Let a registered worker run the tasks leased to it, as a separate process on the same host:
`forge worker run` claims the worker's active leases, runs each task through the standard launch
path (managed worktree, agent, gates, evidence), keeps the lease renewed while the agent runs, and
releases it afterwards. The result waits for operator review, accept, approve, and integrate, as
for any local run.

## Non-goals

- No network transport or authentication: the worker is a local process that shares the project
  directory (P4-M006 and later).
- No automatic lease granting: the operator still grants (`forge lease grant`).
- No change to review or integration. Exact-SHA acceptance of remote results comes later.
- No signal handling: stopping the process stops the worker, and its lease expires on its own.

## Context

P4-M004 made leases operable and made every local run path refuse a leased task. That guard also
refuses the lease holder, so nothing can execute a leased task yet. P0-M013 made the audit log
safe for concurrent writers, which a worker needs: it renews its lease, writing lease evidence,
while its execution writes task evidence.

## Architecture placement

- **Orchestrator:**
  - `LeaseClaim { lease_id, worker_id, generation }`.
  - `launch_leased_process_persisted(..., claim)` shares its implementation with
    `launch_process_persisted`. The lease guard allows a task whose active lease matches the claim
    exactly (lease ID, worker, and generation). It still refuses a task whose paths overlap another
    active lease, and any mismatched, expired, or missing lease.
  - Existing callers are unchanged.
- **Operator:**
  - `leases::claim_lease(root, lease_id, worker, now)`: under the lease lock, it verifies that the
    lease is active and owned by the worker and that its task is `pending`. It appends
    `LeaseRecorded` with `action=claimed` (an audit-only action: the book does not change) and
    returns the claim.
  - A new `worker` module with `run_worker(root, worker_id, adapter, options, report)`:
    1. Verify the worker is registered.
    2. Loop:
       - pick the next claimable lease: this worker's, active now, task `pending` and ready, in
         lease-ID order;
       - claim it;
       - start a renewal thread that renews the lease for its original window every third of that
         window (actor `worker:<id>`);
       - run `launch_leased_process_persisted` with the task's recorded approvals;
       - stop and join the renewal thread;
       - release the lease.
    3. Report each step through a callback.
  - Options: `base_ref`, `once` (at most one task, then return), and `poll` (the wait between
    polls when no lease is claimable).
  - A failed agent or gate still releases the lease. The task is `failed` or `running` for the
    operator, as with any launch.
- **CLI:** `forge worker run <root> <worker-id> (<absolute-executable> | --profile <profile>)
  [--base <ref>] [--once] [--poll-ms <ms>]`. It prints the claim, the standard agent and gate
  report, and the release. With `--once` it exits 0 when idle or successful, and 1 when the task
  failed. Without `--once` it runs until stopped.

## Invariants

- Only the exact lease holder can run a leased task, and only through the worker path.
- A task runs at most once per claim: after launch it is `running` or `failed`, so it can no
  longer be claimed or leased.
- Lease evidence (claimed, renewed, released) and execution evidence share one verified audit
  chain.
- A worker's execution authority is the task contract's: the lease adds none.

## ADRs

ADR-0048 "Same-host worker process": the claim-matching guard, claim evidence, the renewal cadence,
release on every outcome, and stopping by process exit.

## Public API / CLI

`LeaseClaim`, `launch_leased_process_persisted`, `leases::claim_lease`, `worker::run_worker`, and
`forge worker run`.

## Compatibility analysis

Additive. `claimed` is a new `action` value on the existing `LeaseRecorded` kind, so there is no
audit format change.

## Dependency analysis

None. The operator crate already depends on the orchestrator and the adapter types it re-exports.
If `agentforge-adapter` must be named directly, it is an internal edge.

## Expected file boundary

- `.plans/P4-M005-same-host-worker.plan.md`, `.plans/ACTIVE`
- `crates/agentforge-orchestrator/src/lib.rs`, `crates/agentforge-orchestrator/tests/*.rs`
- `crates/agentforge-operator/Cargo.toml`, `crates/agentforge-operator/src/lib.rs`,
  `crates/agentforge-operator/src/leases.rs`, `crates/agentforge-operator/src/worker.rs`,
  `crates/agentforge-operator/tests/*.rs`
- `crates/agentforge-cli/src/main.rs`, `crates/agentforge-cli/tests/*.rs`
- `Cargo.lock`
- `docs/adr/ADR-0048-same-host-worker.md`, `docs/REMOTE_WORKERS.md`, `docs/OPERATIONS.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

- **Orchestrator:** a leased task runs with the matching claim. It is refused, before any side
  effect, with a wrong worker, a wrong generation, a wrong lease ID, or an expired lease. A task
  overlapping another worker's lease is still refused under a claim.
- **Operator `claim_lease`:** it succeeds for the owner and records `claimed`. It refuses another
  worker, a released lease, an expired lease, or a non-pending task, and writes nothing.
- **Operator `run_worker`** (a fixture agent):
  - with `once`, it runs its leased task: the task is `running`, the lease is `released`, and the
    audit order is claimed, execution events, released;
  - it ignores another worker's lease and returns idle;
  - it refuses an unregistered worker;
  - a slow agent with a short lease window gets at least one `renewed` event, and the lease never
    expires during the run;
  - a failing agent still releases the lease.
- **CLI:** `forge worker run --once` end to end through the real binary with the fixture agent.
  An idle `--once` exits 0 with "no claimable leases". The lease list afterwards shows
  `state=released`.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Orchestrator claim path → `claim_lease` → the `run_worker` loop → CLI, with tests at each step.
4. ADR and docs. Full gate plus package preflight. Dogfood: a worker process started in the
   background with a real lease, while the operator watches `forge lease list` and `forge hud`.
5. Push and verify CI (push plus two repeats); close; tag.

## Failure modes

- The worker dies mid-run: renewals stop and the lease expires (the daemon sweep or `lease
  expire`). The task stays `running` with its evidence; the operator cancels or retries, as for any
  interrupted launch.
- A renewal fails (the lock is busy past its wait, or the lease was released by the operator): the
  worker reports it and keeps running. The execution is already authorized by the claim and the
  task state.
- A clock jump: the renewal cadence is a third of the window, and windows are bounded.

## Documentation impact

REMOTE_WORKERS ("Running a same-host worker"), OPERATIONS (the command and a recovery row), and
ADR-0048. README, CHANGELOG, and MILESTONES at closure.

## Quality gates

- `./scripts/gate.sh full` and `./scripts/package-preflight`;
- push-triggered CI plus two dispatched repeats.

## Acceptance criteria

- [ ] `forge worker run` executes its leased tasks through the standard launch path, with claimed,
      renewed, and released evidence.
- [ ] Only the exact lease holder can run a leased task; other paths stay refused.
- [ ] ADR-0048 and docs; CI evidence; closed and tagged.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
