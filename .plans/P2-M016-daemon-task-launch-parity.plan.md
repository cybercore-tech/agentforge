# Plan: P2-M016 — Daemon task-launch parity

Status: Approved
Milestone: P2-M016
Created: 2026-09-21

## Goal

Give detached daemon execution the same safe, low-ceremony task preparation as the P2-M015
foreground pilot. An operator should be able to submit one explicitly configured task to a running
local daemon and have AgentForge validate readiness, resolve the exact base, create or verify the
deterministic managed worktree, record the observation, and then execute through the existing
bounded process path.

## Context

P2-M015 added `forge task launch`, which composes worktree preparation and foreground execution
without making acceptance, review, integration, or retirement implicit. The daemon path still
requires a separate `forge worktree create` command before `forge daemon run`. That split is safe,
but it is an unnecessary operator failure point and makes foreground and detached execution differ
at the most important preparation boundary. P2-M016 closes only that parity gap while preserving
the daemon's loopback, serialization, and cooperative lifecycle model.

## Non-goals

- Do not auto-accept tasks, review diffs, integrate branches, retire worktrees, delete branches,
  reset or clean repositories, or resolve conflicts.
- Do not change task-state meanings, approval semantics, capability policy, audit integrity, or
  exact-head CI policy.
- Do not add multi-task batching, remote workers, hosted review, deployment, or provider-specific
  authority.
- Do not redesign the daemon transport, endpoint identity, lock, timeout, or stop semantics.
- Do not remove the existing explicit worktree commands or change the established `daemon run`
  request contract without a versioned compatibility decision.

## Proposed user-facing contract

Add an explicit daemon launch command that mirrors the foreground contract:

```text
forge daemon launch <root> <task-id> <absolute-executable> [--base <ref>]
forge daemon launch <root> <task-id> --profile <profile> [--base <ref>]
```

The command submits a versioned launch request to a running loopback daemon. The daemon performs
all readiness, capability, approval, exact-base, repository, and worktree ownership checks before
creating or reusing the deterministic managed worktree. It records a durable worktree observation
and invokes the existing persisted process execution path. The response reports bounded execution
termination and recovery commands, while acceptance, diff review, integration, and retirement
remain explicit operator actions.

The existing `forge daemon run` command remains available with its current prepared-worktree
semantics for compatibility. Repeated launch requests verify and reuse an owned clean worktree;
they never adopt unrelated entries or create a second branch.

## Architecture placement

- Reuse the P2-M015 orchestration entry point for worktree preparation and persisted execution.
- Extend the daemon request/response protocol with a versioned launch operation rather than
  duplicating policy or Git logic in the daemon crate.
- Keep daemon serialization at the request handler boundary; one mutating launch or run remains
  active at a time.
- Keep executable and profile values as direct arguments. No shell interpolation or ambient
  command construction is introduced.

## Invariants

- Every validation and approval check completes before worktree creation or agent spawn.
- Base refs resolve to one exact commit and task/path/branch ownership remains deterministic.
- Existing worktrees must be clean and free of unresolved Git operations; unrelated or ambiguous
  entries fail closed.
- Agent success is evidence only. Acceptance, review, integration, and retirement stay separate.
- Failure, timeout, interruption, and audit persistence errors preserve inspectable evidence and do
  not trigger forced cleanup.
- Unsupported or malformed launch frames remain bounded protocol errors.
- The daemon remains loopback-only and cooperative; a client disconnect does not cancel execution.

## Expected file boundary

Implementation may modify only:

- `.plans/P2-M016-daemon-task-launch-parity.plan.md` (approval/completion metadata);
- `crates/agentforge-cli/src/main.rs` and focused daemon-command integration tests;
- `crates/agentforge-daemon/src/lib.rs` and focused protocol/lifecycle tests;
- `crates/agentforge-orchestrator/src/lib.rs` and focused orchestration tests only if a narrow
  reusable launch seam is required;
- `docs/adr/ADR-0028-daemon-task-launch-parity.md`;
- `docs/DAEMON.md`, `docs/ORCHESTRATION.md`, and focused README runbook examples;
- `docs/MILESTONES.md`, `PROJECT_STATE.md`, and `AGENT_HANDOFF.md` only during closure evidence.

No new dependencies, scheduler changes, worktree-policy weakening, provider-specific adapters,
release workflow changes, or unrelated workspace files are authorized.

## Test-first matrix

| Case | Expected evidence |
| --- | --- |
| Valid pending task and executable | One daemon launch creates or verifies one deterministic worktree and executes. |
| Valid named profile | Profile configuration is validated and passed through the same launch seam. |
| Explicit base ref | The exact resolved commit is used and observed. |
| Missing task, dependency, capability, approval, executable, or profile | Rejected before worktree creation or process spawn. |
| Existing owned clean worktree | Reused idempotently without a second branch or path. |
| Dirty, unresolved, ambiguous, or unrelated worktree | Fails closed without cleanup or adoption. |
| Existing `daemon run` request | Current prepared-worktree behavior remains compatible. |
| Malformed/unknown launch frame | Bounded protocol error with no mutation. |
| Disconnect, timeout, failure, and cooperative stop | Durable evidence remains and daemon lifecycle stays bounded. |
| Linux, macOS, Windows | Focused daemon launch and existing workspace suites pass on the supported matrix. |

## Implementation sequence

1. Confirm the P2-M015 launch seam can be reused by daemon execution without duplicating audit or
   worktree authority.
2. Add the versioned daemon launch request, client command, response, and bounded parser checks.
3. Wire launch handling to the existing persisted execution path and add success/rejection,
   repeat, disconnect, and lifecycle fixtures.
4. Document foreground-versus-daemon launch behavior and the explicit acceptance/review/
   integration/retirement recovery sequence.
5. Run the complete local gate, exact implementation CI, then close with exact closure CI and
   evidence for the final SHA.

## Failure classification

- Invalid task, approval, capability, or configuration: controlled policy/usage failure.
- Worktree ownership, dirty state, unresolved operation, or base drift: semantic preflight failure.
- Protocol framing or version mismatch: workflow/governance failure with no mutation.
- Process timeout, interruption, or child failure: execution failure with preserved evidence.
- Filesystem, socket, Git, or hosted-runner behavior: infrastructure failure; verify state before
  repair.
- Audit continuation or persistence failure: durable-state failure; never report a successful
  launch without its evidence boundary.

## Documentation impact

Document the new daemon launch command, its compatibility with `daemon run`, the exact recovery
sequence, and the fact that detached launch does not grant acceptance or integration authority.
Add ADR-0028 for the protocol/versioning and authority-boundary decision.

## Quality gates

- `cargo fmt --all -- --check`.
- `CARGO_TARGET_DIR=/tmp/agentforge-cargo-target ./scripts/gate.sh full`.
- Focused daemon protocol, CLI, orchestrator, worktree, audit, and cross-platform tests.
- `git diff --check` and repository policy checks.
- Exact-commit remote CI green across repository policy, stable, MSRV, CLI smoke, Ubuntu, macOS,
  and Windows.

## Acceptance criteria

- [ ] One explicit daemon launch command prepares and runs one approved task through the existing
      bounded execution path.
- [ ] Foreground and daemon launch share worktree, policy, approval, adapter, state, and audit
      authority without duplicated Git or lifecycle logic.
- [ ] Existing daemon run behavior remains compatible and explicit.
- [ ] Repeat, failure, timeout, disconnect, and malformed-request paths preserve recovery evidence
      without forced cleanup or implicit acceptance/integration.
- [ ] Linux, macOS, and Windows suites cover the new launch path and exact implementation CI is
      green across all supported jobs.
- [ ] Local gate and exact implementation/closure CI evidence are recorded before completion.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
