# Plan: P4-M008 — Remote execution and exact-SHA result import

Status: Complete
Milestone: P4-M008
Created: 2026-09-24
Owner: AgentForge project
Implementer: operator-authored (importing work produced elsewhere is an authority boundary)

## Goal

A worker on another machine runs the task it claimed and returns the result. The coordinator
imports it only after verifying it exactly, runs the task's gates itself, and hands it to normal
review. Together with P4-M007 this completes remote execution:

- **Remote side.** `forge worker remote run` claims over the authenticated channel, checks out the
  exact base commit in its own clone, runs the agent in a managed worktree while renewing the lease,
  commits in-bounds changes, and returns the exit status, bounded logs, and a `git bundle`.
- **Coordinator side.** The new `RESULT` verb verifies everything before any side effect:
  - the lease matches exactly and is active;
  - the bundle is valid, and the fetched commit equals the claimed SHA;
  - the commit descends from the claimed base;
  - every changed path is inside the task's allowed paths and outside its forbidden paths.

  It then creates the task worktree at exactly that commit, records the agent evidence, runs the
  task's gates **locally**, persists the result, and releases the lease. After that come the usual
  `task diff`, `accept`, the post-review merge approval bound to that SHA (P1-M008), and
  `integrate`.

## Non-goals

- No trust in remote gate results: gates always run on the coordinator.
- No automatic accept or integrate.
- No retry of a failed import: the task goes to `failed` with evidence, and the operator decides.
- No streaming of the result; one bounded request.

## Context

P4-M007 delivered authenticated claim, renew, and release, plus the exact contract and base commit.
P1-M008 bound merge approvals to a reviewed SHA, so an imported commit is reviewed and integrated
exactly as it was imported. The local launch path already has gate selection and execution,
evidence recording, and batch audit appends. The import reuses them.

## Architecture placement

- **Orchestrator:** `import_remote_result(root, task_store, audit_store, task_id, RemoteResult)`.
  - Preflight, with no side effects:
    - the task is `pending` and ready;
    - the claim matches the active lease (the P4-M005 guard with `LeaseClaim`);
    - the required gates are configured;
    - the task has no existing managed worktree.
  - With a bundle:
    - `git bundle verify` passes;
    - `git fetch` of the bundle's `refs/heads/agentforge/task/<id>` into
      `refs/agentforge/remote/<lease>` gives exactly `head_commit`;
    - `head` descends from `base`, and `base..head` is not empty;
    - every changed path is allowed and not forbidden.
  - A result with no commits (`head == base`) records the agent evidence, and the task fails with
    "no changes".
  - Side effects:
    - the managed worktree and task branch are created at `head`;
    - `WorktreeObserved` (`mode=remote`), running, `AgentStarted` (adapter `remote:<worker>`), and
      `AgentFinished` are recorded, with remote logs saved as evidence and fields
      `channel=remote`, `worker`, `base_commit`, and `head_commit`;
    - gates run locally in the worktree, with the same evidence and failure transitions as a
      local launch;
    - everything is persisted in one batch.
  - The quarantine ref is deleted afterwards. A verification failure deletes it too and changes
    nothing else.
- **Daemon worker API:**
  - `RESULT <lease> <base> <head> <exit|none> <termination> <stdout-len> <stderr-len>
    <bundle-len>`, followed by the bodies. Limits: 1 MiB per log and 32 MiB for the bundle.
  - Authenticated, and only for the worker's own lease.
  - It runs on its own thread, holding the daemon's execution slot (task-snapshot writes are
    serialized with executions). A busy slot answers `BUSY`, and the worker retries while it keeps
    renewing.
  - After import it releases the lease (`channel=remote`) and answers with the task's resulting
    state and gate summary.
  - `CLAIM` now also refuses a lease whose task lacks a recorded pre-execution approval, and it
    returns the lease window so renewals never depend on the worker's clock.
- **Remote runner:** `worker_api::run_remote_worker`, and in the CLI `forge worker remote run
  --endpoint --worker --secret-file --repo <clone> (<exe> | --profile <p>) [--once] [--poll-ms]`.
  - It ensures the base commit exists in the clone (one `git fetch` if it is missing; otherwise it
    releases the lease and reports).
  - It creates the managed worktree at the base and runs the agent with the task's pre-execution
    approvals (vouched for by the coordinator's claim check). It renews every third of the window.
  - If the worktree is dirty, it checks the changed paths against the contract, then commits them
    as `agentforge: remote result for <task>`. On a boundary violation it releases the lease and
    sends nothing.
  - It bundles `base..agentforge/task/<id>`, sends `RESULT`, retries on `BUSY`, and retires the
    local worktree (the branch is kept).

## Invariants

- Nothing from a remote worker reaches a task branch unless its exact SHA, ancestry, and path
  boundary are verified.
- Gates that decide the task's state always run on the coordinator.
- Import and local executions never write the task snapshot concurrently inside the daemon.
- The imported commit is exactly the one reviewed and integrated (P1-M008 binding).

## ADRs

ADR-0051 "Remote execution and exact-SHA import": bundle transfer, the verification order,
quarantine refs, local gates, the slot, and remote auto-commit.

## Public API / CLI

`RemoteResult`, `import_remote_result`, the `RESULT` verb, the `CLAIMED` window field,
`run_remote_worker`, and `forge worker remote run`.

## Compatibility analysis

The AFW1 `CLAIMED` response gains a trailing `window_ms`. It is pre-1.0, same-build, and was
released only one milestone ago; the protocol doc records it. Everything else is additive.

## Dependency analysis

None.

## Expected file boundary

- `.plans/P4-M008-remote-execution.plan.md`, `.plans/ACTIVE`
- `crates/agentforge-orchestrator/src/lib.rs`, `crates/agentforge-orchestrator/tests/*.rs`
- `crates/agentforge-daemon/src/lib.rs`, `crates/agentforge-daemon/src/worker_api.rs`,
  `crates/agentforge-daemon/tests/*.rs`
- `crates/agentforge-operator/src/*.rs`, `crates/agentforge-operator/tests/*.rs`
- `crates/agentforge-cli/src/main.rs`, `crates/agentforge-cli/tests/*.rs`
- `Cargo.lock`
- `docs/adr/ADR-0051-remote-execution.md`, `docs/REMOTE_WORKERS.md`, `docs/OPERATIONS.md`,
  `docs/DAEMON.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

- **Orchestrator `import_remote_result`** (a real coordinator repo, plus a separate clone making the
  bundle):
  - a valid result gives a worktree at the exact head, the task `running`, and evidence with
    `channel=remote`;
  - a failing gate gives the task `failed`;
  - rejected, with no state, audit, branch, or quarantine ref left behind:
    - a head SHA that does not match the bundle;
    - a head that is not a descendant of the base;
    - a path outside the allowed paths, or a forbidden path;
    - a wrong or expired claim;
    - a corrupt bundle;
    - an existing worktree;
  - no commits gives a failed task with "no changes".
- **Worker API `RESULT`:** authenticated and owner-only; an oversize body is refused; `BUSY` while
  the slot is held; the lease is released after import. `CLAIM` refuses a task missing a
  pre-execution approval and returns `window_ms`.
- **Remote runner end to end** (Unix): a coordinator repo with `forged` and the worker API, plus a
  separate clone as the worker host. The fixture agent's uncommitted file is auto-committed and
  imported. The coordinator task branch equals the reported SHA, gates ran on the coordinator, and
  `task diff`, `accept`, `approve`, and `integrate` land it. A boundary-violating agent causes a
  release and no import.
- **GhostPort dogfood:** the same flow through a real tunnel.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Orchestrator import with tests, then the daemon `RESULT` and `CLAIM` changes, then the remote
   runner and CLI.
4. ADR and docs. Full gate plus package preflight. The GhostPort dogfood, through to `integrate`.
5. Push and verify CI (push plus two repeats); close; tag.

## Failure modes

- Import verification fails: the result is rejected with the reason, and the lease is released by
  the worker. The task stays `pending` (it is not auto-re-dispatched), and the operator decides.
- The worker dies after sending: the import completes independently, and the lease is released by
  the coordinator.
- A large bundle: it is bounded at 32 MiB. Larger work needs a shared remote (future).

## Documentation impact

REMOTE_WORKERS ("Running tasks remotely"), DAEMON (RESULT and the slot), OPERATIONS, and ADR-0051.
README, CHANGELOG, and MILESTONES at closure.

## Quality gates

- `./scripts/gate.sh full` and `./scripts/package-preflight`;
- push-triggered CI plus two dispatched repeats.

## Acceptance criteria

- [x] A remote worker runs its claimed task and returns a bundle, and the coordinator imports it
      only at the verified exact SHA with in-bounds paths.
- [x] Gates run on the coordinator, and the result follows normal review through to `integrate`.
- [x] Rejections leave no state, audit, or refs behind.
- [x] Verified through GhostPort; ADR-0051 and docs; CI evidence; closed and tagged.

## Completion record

Implementation commit: `42eb636`
CI run: `36095220162` (push), plus `36095589777` and `36095597117` (dispatched)
CI result: green on all seven jobs in all three runs
Completed: 2026-09-24
Notes: Operator-authored as planned, with no amendments. The dogfood ran through real GhostPort
v0.1.1 processes, with a live `forged` serving the worker API and a separate clone as the worker
host:
- the operator granted the lease;
- `forge worker remote run` claimed over the tunnel, ran the fixture agent, and auto-committed;
- the coordinator verified and imported head `211563f` and ran its own gate (1/1);
- `task diff`, `accept`, `approve` (bound to `211563f`), and `integrate` put exactly that commit,
  authored by the remote worker, on `main`.

The import, CLI, and worker API tests passed on their first run.
