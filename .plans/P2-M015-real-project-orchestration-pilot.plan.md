# Plan: P2-M015 — Real-project orchestration pilot

Status: Complete
Milestone: P2-M015
Created: 2026-09-21

## Goal

Reduce the manual ceremony required to run one real project task while preserving AgentForge's
approval, worktree, execution, audit, review, and integration boundaries. The pilot should let an
operator launch a prepared task from a clean project with one explicit command, then recover from
success or failure using visible, durable next steps.

## Context

P2-M014 completed safe review and fast-forward-only integration, but the normal operator journey
still requires separate worktree preparation before `forge run`. The existing direct run path is
safe and tested, yet a real project pilot should exercise the full path with less ceremony and
without making acceptance, integration, or retirement implicit. This increment is intentionally
focused on one foreground task; multi-task daemon scheduling remains a later milestone.

## Non-goals

- Do not change task-state meanings, approval semantics, capability policy, audit integrity, or
  exact-head CI policy.
- Do not auto-accept successful work, integrate branches, retire worktrees, delete branches, or
  clean/reset repositories.
- Do not add conflict resolution, rebases, force pushes, pull requests, hosted review, deployment,
  or remote worker behavior.
- Do not redesign the daemon protocol, scheduler batch semantics, HUD rendering, intake format, or
  release workflow in this pilot.
- Do not add provider-specific authority; executable/profile selection remains explicit and direct.

## Proposed user-facing contract

Add an explicit foreground pilot command:

```text
forge task launch <root> <task-id> <absolute-executable> [--base <ref>] [--interactive] [--pty]
forge task launch <root> <task-id> --profile <profile> [--base <ref>] [--interactive] [--pty]
```

The command validates the task snapshot, readiness, capabilities, approvals, repository root, and
agent configuration before creating anything. It resolves the base ref exactly (default `HEAD`),
creates or verifies the deterministic managed worktree, and invokes the existing bounded process
execution path. It leaves the task in the existing execution/review state and leaves the worktree
available for inspection, acceptance, diff review, integration, and explicit retirement.

If execution fails or times out, the command preserves the failed task evidence and managed
worktree and prints bounded recovery guidance (`task inspect`, `task retry` when permitted,
`worktree inspect`, and the relevant audit/HUD commands). A repeated launch never creates a second
branch or silently adopts an unrelated worktree; it either verifies the existing owned worktree or
fails closed with a precise recovery action.

## Architecture

- Add a small orchestration entry point that composes existing worktree creation/inspection and
  `execute_process_persisted`; keep process execution, policy, and audit authority in their current
  crates.
- Reuse `WorktreeManager::resolve_base`, `create`, and `inspect` so base commits and ownership
  remain exact and deterministic.
- Keep all executable, profile, environment, and task-controlled values as direct process
  arguments; no shell interpolation or ambient command construction is introduced.
- Record the worktree preparation observation through the existing versioned audit event shape and
  preserve the existing attempt sequence/digest continuation behavior.
- Keep interactive and PTY flags opt-in and routed through the existing adapter configuration; the
  pilot must not create a second terminal implementation.

## Safety invariants

- All validation and approval checks happen before worktree creation or agent spawn.
- Base resolution is exact and branch/path identities remain deterministic.
- Existing managed worktrees must pass ownership, cleanliness, and unresolved-operation checks;
  unrelated or ambiguous entries are never adopted.
- Agent success is evidence only. Acceptance, integration, and retirement remain independent human
  actions.
- Agent failure, timeout, interruption, or audit persistence failure preserves durable evidence and
  never triggers forced cleanup.
- Repeated invocation is bounded and idempotent at the worktree boundary.

## Expected file boundary

Implementation may modify only:

- `.plans/P2-M015-real-project-orchestration-pilot.plan.md` (approval/completion metadata);
- `crates/agentforge-cli/src/main.rs` and focused CLI integration tests;
- `crates/agentforge-orchestrator/src/lib.rs` and focused orchestration tests;
- `crates/agentforge-audit/src/lib.rs` and focused audit tests only if existing observation fields
  need a bounded extension;
- `docs/adr/ADR-0027-real-project-orchestration-pilot.md`;
- `docs/ORCHESTRATION.md`, `docs/DAEMON.md`, and focused README command/runbook examples;
- `docs/MILESTONES.md`, `PROJECT_STATE.md`, and `AGENT_HANDOFF.md` only during closure evidence.

No new dependencies, scheduler changes, daemon protocol changes, worktree policy weakening,
provider-specific adapters, release workflow changes, or unrelated workspace files are authorized.

## Test-first matrix

| Case | Expected evidence |
| --- | --- |
| Valid pending task and executable | One deterministic worktree is created and the existing execution path starts. |
| Explicit base ref | Exact base commit is recorded and used; symbolic drift is not guessed. |
| Existing owned worktree | Repeated launch verifies and reuses it without creating another branch. |
| Missing task, dependency, capability, or approval | Rejected before worktree creation or process spawn. |
| Missing/invalid executable or profile | Bounded configuration error with no task/worktree mutation. |
| Agent success | Existing running/review state and bounded audit evidence are preserved; no acceptance or merge. |
| Agent failure/timeout/interruption | Failure evidence is durable; task/worktree remain inspectable for repair forward. |
| Dirty, unresolved, ambiguous, or unrelated worktree | Launch fails closed without cleanup or adoption. |
| Interactive and PTY modes | Existing terminal behavior is reused and remains fail-closed outside a real terminal. |
| Linux, macOS, Windows | Focused pilot and existing workspace suites pass on the supported matrix. |

## Implementation sequence

1. Confirm the current direct run, profile, PTY, audit, and worktree APIs provide the required
   composition points without adding authority or dependencies.
2. Add the bounded orchestration entry point and deterministic CLI parser for `task launch`.
3. Add recovery-oriented output and focused fixtures for success, failure, repeat, and preflight
   rejection; keep acceptance/integration/retirement separate.
4. Document a disposable real-project pilot runbook using both Codex- and Claude-style executables.
5. Run the complete local gate, exact implementation CI, then close the milestone with exact
   closure CI and evidence.

## Failure classification

- Invalid task, approval, capability, or configuration: controlled policy/usage failure.
- Worktree ownership, dirty state, unresolved operation, or base drift: semantic preflight failure.
- Process timeout, interruption, or child failure: execution failure with preserved evidence.
- Filesystem, Git, or platform behavior: infrastructure failure; verify state before repair.
- Audit continuation or persistence failure: durable-state failure; never report a successful launch
  without its evidence boundary.

## Quality gates

- `cargo fmt --all -- --check`.
- `CARGO_TARGET_DIR=/tmp/agentforge-cargo-target ./scripts/gate.sh full`.
- Focused CLI, orchestrator, worktree, audit, and cross-platform pilot tests.
- `git diff --check` and repository policy checks.
- Exact-commit remote CI green across repository policy, stable, MSRV, CLI smoke, Ubuntu, macOS,
  and Windows.

## Acceptance criteria

- [x] One explicit `forge task launch` command can run a real approved task from a clean project.
- [x] The command composes existing worktree, policy, adapter, state, and audit boundaries without
      duplicating authority.
- [x] Repeated launch, failure, timeout, and interruption paths preserve inspectable recovery state.
- [x] Acceptance, review, integration, branch preservation, and retirement remain explicit.
- [x] Foreground, interactive, PTY, and profile modes remain compatible and bounded.
- [x] A disposable real-project runbook is documented and exercised with at least two local agent
      executable styles.
- [x] Linux, macOS, and Windows tests cover the pilot path through the existing matrix; the focused
      pilot suites pass locally and the exact implementation CI is green across all supported jobs.
- [x] Local gate and exact implementation CI are recorded; closure evidence follows in the closure
      checkpoint.

## Completion record

Implementation commit: `8992da820162e3bd410841dd8eeddd8e5f04a935`.
Exact implementation CI: `35549918749` — all seven jobs green across repository policy, stable,
MSRV, CLI smoke, Ubuntu, macOS, and Windows.
Closure commit: pending.
Exact closure CI: pending.
