# Plan: P2-M005 — Local daemon and dogfooding

Status: Approved
Milestone: P2-M005
Created: 2026-09-20

## Goal

Turn the existing persisted single-task orchestration path into a usable local runtime by replacing
the `forged` identity placeholder with a bounded, loopback-only daemon and proving one real operator
workflow from intake through acceptance. Failed and interrupted executions must remain durable and
auditable across process boundaries.

## Scope

- Implement a standard-library-only `forged` service with explicit `serve`, `status`, `run`, and
  `stop` lifecycle behavior.
- Use a versioned, bounded request/response protocol over an explicitly loopback-bound local
  transport; never expose a wildcard or remote listener.
- Persist daemon endpoint, process identity, and lifecycle state under `.forge/daemon/` using
  atomic writes and stale-instance detection. Refuse ambiguous or concurrently active instances.
- Serialize one mutating task execution at a time through the existing orchestrator and preserve
  task-owned worktree, policy, approval, gate, and audit boundaries.
- Repair persisted execution so transitions and audit records for adapter failures, timeouts, and
  interrupted requests are saved before the error is returned.
- Add CLI commands for daemon lifecycle and daemon-backed task execution while retaining the
  existing direct `forge run` path.
- Add a real temporary-repository dogfooding test and operator documentation for starting the
  daemon, submitting one approved task, inspecting evidence, accepting it, and stopping safely.

## Non-goals

- No cloud service, remote worker, MCP gateway, provider marketplace, or multi-host protocol.
- No automatic task acceptance, merge, branch deletion, forced worktree cleanup, or hidden
  capability/approval grants.
- No shell interpolation, arbitrary command strings, unbounded requests, or unbounded output.
- No external Rust dependencies or platform-specific transport requirement.
- No artifact signing/provenance work; release packaging remains P2-M004 behavior.

## Context

The project already has durable task snapshots, audit logs, policy evaluation, worktree isolation,
the process adapter, gates, operator actions, and a direct `forge run` command. The `forged` binary
still prints a placeholder message. In addition, `execute_process_persisted` currently propagates an
adapter error before saving the in-memory failed transition and its audit records. This milestone
connects those existing contracts without changing their authority boundaries.

## Architecture placement

- `agentforge-daemon` owns the local service lifecycle, endpoint protocol, instance lock, and
  request dispatch.
- `agentforge-orchestrator` owns execution ordering and durable success/failure persistence.
- `agentforge-cli` owns operator-facing lifecycle and run commands; it remains a thin caller of
  library APIs.
- `.forge/daemon/` is project-local operational state and is never treated as task authority.
- Existing task, policy, worktree, adapter, gate, audit, and operator crates remain the sources of
  truth for their respective boundaries.

## Data flow

1. Operator starts `forged serve --root <project>`; the daemon validates the repository and writes
   a versioned endpoint/identity record atomically after acquiring its instance lock.
2. `forge daemon status|run|stop` reads and validates that record, connects only to the recorded
   loopback endpoint, and applies bounded protocol framing.
3. A `run` request verifies task identity, durable state, required approvals, and executable
   configuration before invoking the existing persisted orchestrator.
4. The orchestrator records `Running`, adapter result/failure, and audit evidence durably before
   returning a bounded response. Successful work remains pending independent acceptance.
5. `forge task accept|cancel|retry` remains the explicit operator boundary; daemon shutdown never
   mutates task acceptance or deletes worktrees.

## Invariants

- Only one daemon instance may own a project at a time; stale or malformed identity state fails
  closed and is not silently adopted.
- Only loopback connections to the recorded endpoint are accepted.
- Requests, responses, task IDs, executable paths, and diagnostic data have explicit size limits.
- Daemon execution cannot bypass capabilities, approvals, policy, worktree ownership, gates, or
  independent acceptance.
- A failed, timed-out, or interrupted execution leaves a durable task state and ordered audit trail
  (or an explicit durable persistence error) before the caller observes failure.
- Stop is cooperative and non-forced; active work is reported and preserved rather than killed or
  cleaned destructively.
- Existing direct CLI behavior remains compatible.

## ADRs

- ADR-0002 — Git worktrees are the task isolation boundary.
- ADR-0010 — Deterministic managed worktree lifecycle.
- ADR-0019 — Explicit durable single-task orchestration loop.
- A new ADR is not required unless implementation selects a transport or lifecycle policy outside
  this plan's loopback, bounded, cooperative boundary.

## Public API / CLI

- `forged serve --root <root> [--bind 127.0.0.1:0]`
- `forge daemon status <root>`
- `forge daemon run <root> <task-id> <absolute-executable>`
- `forge daemon stop <root>`
- Existing `forge run <root> <task-id> <absolute-executable>` remains supported.

## Compatibility analysis

- Existing `.forge/state/tasks.snapshot` and audit log formats remain backward compatible.
- Existing task lifecycle and approval names remain unchanged.
- New daemon metadata is versioned and can be ignored by older clients; malformed metadata is
  rejected rather than guessed.
- The daemon is optional; direct CLI execution remains available without a running service.

## Dependency analysis

- Use only the Rust standard library and existing workspace crates.
- Do not add network, serialization, process-management, or runtime dependencies.

## Expected file boundary

- `.plans/P2-M005-daemon-dogfooding.plan.md`, `.plans/ACTIVE`
- `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, `docs/MILESTONES.md`
- `crates/agentforge-daemon/Cargo.toml`, `crates/agentforge-daemon/src/main.rs`,
  `crates/agentforge-daemon/src/lib.rs`, and daemon tests
- `crates/agentforge-cli/src/main.rs` and daemon CLI integration tests
- `crates/agentforge-orchestrator/src/lib.rs` and persisted execution tests
- `docs/DAEMON.md`, `docs/ORCHESTRATION.md`, and `README.md`
- `Cargo.lock` only if an existing workspace manifest edit requires lockfile regeneration; no new
  dependencies are authorized.

## Test-first matrix

| Area | Required evidence |
| --- | --- |
| Protocol | version, framing, size limits, malformed requests, unknown commands, and bounded errors |
| Lifecycle | start, status, stop, stale identity, duplicate instance, cooperative stop, and restart |
| Execution | approval/policy/worktree checks remain enforced; only one task mutates at a time |
| Durability | adapter failure, timeout, interrupted request, and persistence failure retain evidence |
| CLI | daemon lifecycle and run commands return deterministic output and exit codes |
| Dogfooding | temporary repository completes intake → approval → daemon run → audit/HUD inspection → explicit acceptance |
| Compatibility | direct `forge run`, existing snapshots, existing tests, formatting, Clippy, and docs checks |

## Implementation sequence

1. Commit this approved plan and active pointer as a plan-only checkpoint.
2. Add daemon protocol, endpoint/identity state, lifecycle lock, and bounded request dispatch.
3. Refactor persisted orchestration to save success and failure evidence on every terminal adapter
   outcome, preserving explicit error reporting.
4. Wire CLI daemon lifecycle/run commands without removing direct execution.
5. Add focused unit/integration tests and the temporary-repository dogfooding flow.
6. Document local daemon operation, recovery, limits, and explicit acceptance boundaries.
7. Run the full local gate and exact-head CI for the implementation commit.
8. Close the milestone with separate documentation and exact-SHA post-merge evidence.

## Failure modes

- Missing, malformed, stale, or conflicting daemon metadata: fail closed with recovery guidance.
- Port bind/connect, lock, or child-process failure: preserve state and return bounded diagnostics.
- Invalid task, missing approval, dirty worktree, policy denial, timeout, or nonzero exit: persist
  the appropriate evidence and leave acceptance to the operator.
- Client disconnect: daemon records interruption and keeps worktree/state inspectable.
- Shutdown during active work: refuse or defer shutdown cooperatively; never force cleanup.

## Documentation impact

- Add `docs/DAEMON.md` with lifecycle commands, protocol limits, recovery, and local-only security.
- Update `docs/ORCHESTRATION.md`, README command examples, capability notes, and roadmap status.

## Quality gates

- `CARGO_TARGET_DIR=/tmp/agentforge-cargo-target ./scripts/gate.sh full`
- Exact-head GitHub CI for the implementation commit and closure/post-merge commits.
- Temporary-repository integration tests must not use fixed external paths or shared mutable state.

## Acceptance criteria

- [ ] `forged serve` provides a bounded, loopback-only, versioned local service with deterministic
      status/run/stop lifecycle behavior.
- [ ] Duplicate, stale, malformed, and active daemon instances are handled fail-closed without
      forced process or worktree cleanup.
- [ ] Daemon-backed execution preserves all existing policy, approval, worktree, gate, audit, and
      independent-acceptance boundaries and serializes state mutation.
- [ ] Adapter failures, timeouts, and interrupted requests persist task state and audit evidence
      before returning failure.
- [ ] `forge daemon status|run|stop` works while existing direct `forge run` remains compatible.
- [ ] A real temporary-repository dogfooding test completes intake through explicit acceptance.
- [ ] Documentation describes operation, recovery, limits, and security boundaries.
- [ ] Full local gate and exact-head CI pass for implementation and closure commits.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
