# Plan: P2-M006 — Real-agent dogfooding and operator workflow

Status: Approved
Milestone: P2-M006
Created: 2026-09-20

## Goal

Turn the validated local daemon path into a practical first-use workflow by exercising it with a
real configured executable, exposing safe operator commands for managed worktree preparation, and
proving recovery behavior across process boundaries. The workflow must retain explicit approval,
task-owned isolation, bounded evidence, durable state, and independent acceptance.

## Non-goals

- No automatic approval, acceptance, merge, branch deletion, or forced worktree cleanup.
- No provider-specific SDK, cloud service, MCP gateway, remote worker, or external Rust dependency.
- No redesign of the daemon protocol, task contract, policy authority, or audit format.
- No Windows Git verbatim-path repair in this milestone; that remains a separately scoped
  cross-platform worktree milestone.
- No artifact signing or provenance attestations; release hardening remains separate.

## Context

P2-M005 delivered a bounded loopback-only `forged` service, daemon-backed execution, durable
adapter-failure persistence, HUD inspection, and a temporary-repository test. That test uses the
`forge` binary as a portable executable; it proves the orchestration boundaries but does not yet
exercise a realistic configured coding-agent process. Worktree creation is currently a crate API,
so an operator must assemble part of the approved workflow outside the CLI.

## Architecture placement

- `agentforge-cli` owns thin, explicit operator commands for preparing and inspecting managed
  worktrees; it does not become a second source of worktree authority.
- `agentforge-worktree` remains the source of truth for Git discovery, ownership, dirty state, and
  conservative retirement.
- `agentforge-daemon` continues to own local lifecycle, bounded dispatch, and serialized execution.
- `agentforge-adapter` remains provider-neutral; tests use a deterministic executable fixture rather
  than a provider-specific integration.
- Existing task, policy, operator, orchestrator, audit, and HUD crates remain authoritative for
  their current boundaries.

## Data flow

1. The operator initializes a temporary or real project and creates an explicit task contract.
2. `forge worktree create|inspect|list|retire` delegates to the managed worktree API and reports
   bounded, deterministic evidence without accepting or deleting task branches.
3. The operator records required approval, starts `forged`, and submits the task with a configured
   executable through `forge daemon run`.
4. The executable runs in the verified task worktree with the existing policy, adapter, gate,
   audit, and durable-state boundaries.
5. The operator inspects HUD/task/audit evidence, explicitly accepts or retries the result, stops
   the daemon cooperatively, and retires the clean worktree when appropriate.
6. Restart, disconnect, timeout, and stale-metadata scenarios leave inspectable state and bounded
   recovery guidance rather than silently adopting or cleaning anything.

## Invariants

- Worktree commands cannot operate outside the deterministic managed root or alter task branches.
- Worktree retirement remains non-forced and refuses dirty or unresolved-operation states.
- Daemon execution remains single-task serialized and cannot grant acceptance authority.
- The configured executable is passed as direct arguments with bounded output and deadlines; no
  shell interpolation is introduced.
- Failed, interrupted, or timed-out execution remains durable and auditable before failure is
  reported whenever the persistence boundary is available.
- Existing `forge run` behavior and all current task/policy/audit formats remain compatible.
- Operator output is deterministic, bounded, and safe to inspect in scripts or a terminal HUD.

## ADRs

- ADR-0002 — Git worktrees are the task isolation boundary.
- ADR-0010 — Deterministic managed worktree lifecycle.
- ADR-0019 — Explicit durable single-task orchestration loop.
- No new ADR is expected unless the CLI introduces a lifecycle policy outside the existing
  conservative worktree and loopback-daemon boundaries.

## Public API / CLI

- `forge worktree create <root> <task-id> <base-ref>`
- `forge worktree inspect <root> <task-id>`
- `forge worktree list <root>`
- `forge worktree retire <root> <task-id>`
- Existing `forge daemon status|run|stop`, `forge task`, `forge hud`, and direct `forge run`
  commands remain supported.

## Compatibility analysis

- Existing `.forge/state/tasks.snapshot`, audit records, daemon metadata, and task transitions are
  unchanged.
- Worktree commands are additive and delegate to the already tested manager; malformed task IDs,
  missing repositories, ownership mismatches, dirty state, and unresolved operations fail closed.
- The deterministic executable fixture is test-only and does not change provider-neutral adapter
  contracts.
- Unix dogfooding remains the supported worktree-backed integration path while the known Windows
  verbatim temporary-path limitation is tracked separately.

## Dependency analysis

- Use the Rust standard library and existing workspace crates only.
- No new runtime, serialization, process-management, network, or provider dependencies.

## Expected file boundary

- `.plans/P2-M006-real-agent-dogfooding.plan.md`, `.plans/ACTIVE`
- `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, `docs/MILESTONES.md`
- `crates/agentforge-cli/src/main.rs`, `crates/agentforge-cli/tests/daemon_commands.rs`, and
  additive CLI worktree integration tests
- `docs/DAEMON.md`, `docs/ORCHESTRATION.md`, and `README.md`
- Daemon test files only if required for recovery or disconnect evidence
- `Cargo.lock` only if an existing workspace manifest edit requires lockfile regeneration; no new
  dependencies are authorized.

## Test-first matrix

| Area | Required evidence |
| --- | --- |
| Worktree CLI | create, inspect, list, retire, deterministic output, and fail-closed ownership/dirty checks |
| Real executable | deterministic fixture modifies only its task worktree and returns bounded evidence |
| Daemon workflow | intake → worktree preparation → approval → daemon run → HUD/audit inspection → acceptance |
| Recovery | restart, stale metadata, client disconnect, timeout, and cooperative stop preserve evidence |
| Compatibility | direct `forge run`, existing daemon lifecycle, existing snapshots/audit logs, and unchanged task transitions |
| Quality | formatting, Clippy, unit/integration tests, documentation tests, full gate, and exact-head CI |

## Implementation sequence

1. Commit this draft as a plan-only checkpoint; approval and activation remain separate.
2. After approval, add explicit CLI worktree lifecycle commands backed only by `WorktreeManager`.
3. Add a deterministic real-executable fixture and extend temporary-repository dogfooding to use
   it without weakening policy or acceptance boundaries.
4. Add recovery/disconnect/restart evidence and bounded operator diagnostics where gaps remain.
5. Update daemon/orchestration documentation with a complete first-use workflow and recovery guide.
6. Run the full local gate and exact-head CI for the implementation commit.
7. Close the milestone with separate documentation and exact-SHA post-closure evidence.

## Failure modes

- Invalid command shape, task ID, root, base ref, or executable: deterministic usage/error output;
  no mutation.
- Missing or mismatched managed worktree: daemon execution refuses to start and preserves evidence.
- Dirty or unresolved worktree retirement: refusal with inspectable status; no force/remove/reset.
- Agent timeout, nonzero exit, output limit, disconnect, or daemon restart: durable failed/interrupted
  state and ordered audit evidence, or an explicit persistence error.
- Duplicate or stale daemon identity: fail closed with recovery guidance; never silently adopt it.
- Unsupported Windows worktree path behavior: remains documented and excluded from this milestone's
  worktree-backed fixture until separately repaired.

## Documentation impact

- Add complete copy/paste operator examples to `docs/DAEMON.md`.
- Document worktree lifecycle commands, safe retirement, and recovery boundaries.
- Update README capability, command, and roadmap sections after implementation.
- Record the known Windows limitation and the exact validation scope.

## Quality gates

- `CARGO_TARGET_DIR=/tmp/agentforge-cargo-target ./scripts/gate.sh full`
- Exact-head GitHub CI for implementation and closure commits.
- Temporary repositories use unique paths and retire managed worktrees before cleanup.
- No hooks or validation may be bypassed.

## Acceptance criteria

- [ ] Operators can prepare, inspect, list, and safely retire a managed worktree through explicit
      CLI commands with deterministic bounded output.
- [ ] A deterministic configured executable performs a real task-worktree mutation through the
      daemon while existing policy, approval, gate, audit, and acceptance boundaries remain active.
- [ ] A temporary-repository dogfooding test covers intake through explicit acceptance using the
      real executable fixture rather than the `forge` binary as a stand-in.
- [ ] Restart, disconnect, timeout, stale metadata, and cooperative stop behavior preserve durable
      task/audit evidence and fail closed when recovery is ambiguous.
- [ ] Existing direct execution, daemon lifecycle commands, state formats, and cross-platform CI
      behavior remain compatible.
- [ ] Documentation provides a complete first-use workflow and recovery guidance.
- [ ] Full local gate and exact-head CI pass for implementation and closure checkpoints.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
