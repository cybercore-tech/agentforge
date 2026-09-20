# Plan: P2-M007 — Cross-platform agent operations

Status: Approved
Milestone: P2-M007
Created: 2026-09-20

## Goal

Make the first-use workflow practical on all supported CI platforms by repairing Windows managed
worktree path handling, adding explicit local-agent profiles and validation, and providing bounded
daemon supervision and recovery commands. The milestone must preserve task ownership, approval,
durable evidence, conservative cleanup, and provider-neutral execution.

## Scope

1. **Windows worktree support** — make canonical and Git-reported paths comparable on Windows,
   preserve no-shell argument passing, and run the managed worktree lifecycle and daemon-backed
   dogfooding tests on Windows where the platform can support them.
2. **Agent onboarding** — add a project-local, bounded agent profile/configuration format and CLI
   commands that validate an explicitly configured executable, arguments, environment policy, and
   timeout without granting new authority or introducing provider-specific dependencies.
3. **Daemon supervision and recovery** — add explicit start/status/stop/restart or equivalent
   operator controls, bounded readiness/exit diagnostics, and fail-closed stale-instance recovery;
   do not silently adopt, kill, or clean another daemon's state.

## Non-goals

- No artifact signing or provenance attestations.
- No cloud provider SDKs, MCP gateway, remote worker, or external Rust dependency.
- No automatic approval, acceptance, merge, branch deletion, forced worktree removal, `git clean`,
  or `git reset --hard`.
- No general-purpose process supervisor, service-manager integration, or privileged installation.
- No redesign of task, policy, audit, or daemon wire-format contracts unless required for a
  backward-compatible versioned extension.

## Architecture placement

- `agentforge-worktree` remains authoritative for Git discovery, ownership, and retirement; its
  path normalization is extended only to resolve platform-equivalent canonical forms.
- `agentforge-cli` owns explicit profile and daemon operator commands and bounded diagnostics.
- `agentforge-daemon` owns local lifecycle identity, readiness, cooperative stop, and recovery
  boundaries; it never adopts ambiguous metadata or kills unknown processes.
- `agentforge-adapter` consumes a validated profile as direct executable arguments and keeps output,
  timeout, environment, and approval boundaries unchanged.
- Project-local `.forge/` files remain durable configuration/evidence; secrets are not persisted
  by this milestone.

## Proposed operator surface

- `forge agent list <root>`
- `forge agent validate <root> <profile>`
- `forge agent inspect <root> <profile>`
- `forge daemon start <root> [--bind <loopback>] [--foreground]`
- `forge daemon status <root>`
- `forge daemon stop <root>`
- `forge daemon restart <root>`

The exact command names may be reduced during implementation if an equivalent explicit, bounded
surface is clearer. Existing `forge run`, `forge daemon run`, and `forged serve` remain compatible.

## Configuration contract

Agent profiles must be versioned, bounded, non-overwriting, and explicit about:

- profile identifier and executable path;
- direct argument vector (no shell string);
- allowed environment names or a cleared-environment default;
- timeout and output limits within safe maxima;
- optional working-directory mode restricted to the verified task worktree.

Profile validation must reject missing executables, relative paths where an absolute path is
required, duplicate or malformed fields, unsafe environment requests, and out-of-range limits.
Profiles grant no task capability and cannot bypass task approval or worktree verification.

## Recovery contract

- Start waits for a bounded, verified endpoint publication or returns a deterministic diagnostic.
- Status distinguishes running, absent, malformed, and stale metadata.
- Stop remains cooperative and reports if the daemon does not exit within the bounded wait.
- Restart is stop-then-start only after verified ownership; stale metadata requires an explicit
  operator-confirmed recovery action and is never silently removed.
- No command kills a PID merely because it appears in metadata.
- Daemon state and task/audit evidence remain inspectable after crashes, disconnects, and timeouts.

## Expected file boundary

- `.plans/P2-M007-cross-platform-agent-operations.plan.md`, `.plans/ACTIVE`
- `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, `docs/MILESTONES.md`, `README.md`
- `crates/agentforge-worktree/src/lib.rs` and platform-specific worktree tests
- `crates/agentforge-cli/src/main.rs` and additive CLI integration tests
- `crates/agentforge-daemon/src/lib.rs`, `src/main.rs`, and daemon tests
- `crates/agentforge-adapter/src/lib.rs` and adapter/profile tests if required
- `docs/DAEMON.md`, `docs/WORKTREE_ISOLATION.md`, and a new agent configuration guide if needed
- `Cargo.lock` only if an existing workspace manifest change requires regeneration; no new
  dependencies are authorized.

## Test-first matrix

| Area | Required evidence |
| --- | --- |
| Windows worktrees | create, inspect, list, dirty refusal, retire, branch preservation on Windows CI |
| Path normalization | canonical `/private`/symlink and Windows verbatim/non-verbatim equivalent paths |
| Agent profiles | valid profile, malformed profile, executable/argument validation, bounded limits, no shell interpolation |
| Onboarding CLI | list/inspect/validate output is deterministic, bounded, and non-mutating |
| Daemon supervision | start readiness, status states, cooperative stop, bounded restart, stale metadata refusal |
| Compatibility | existing direct run, daemon run, HUD, snapshots, audit records, and task transitions |
| Quality | formatting, Clippy, unit/integration tests, documentation checks, full gate, exact-head CI |

## Implementation sequence

1. Commit this draft as a plan-only checkpoint; approval and activation remain separate.
2. Repair platform-equivalent worktree path handling and remove the Windows exclusion where tests
   demonstrate safe support.
3. Add versioned local agent profiles, validation, CLI inspection, and adapter integration.
4. Add bounded daemon start/restart/readiness/recovery controls and cross-platform tests.
5. Update first-use and recovery documentation, then run the full gate and exact-head CI.
6. Close the milestone with separate documentation and exact-SHA post-closure evidence.

## Quality gates

- `CARGO_TARGET_DIR=/tmp/agentforge-cargo-target ./scripts/gate.sh full`
- Exact-head GitHub CI for implementation and closure commits.
- Windows, macOS, and Linux portable tests must remain green.
- No hooks or validation may be bypassed.

## Acceptance criteria

- [ ] Managed worktree create/inspect/list/retire lifecycle passes on Windows CI without weakening
      ownership, path, dirty-state, or non-forced-retirement guarantees.
- [ ] Operators can define, validate, inspect, and select a local agent profile using direct
      executable arguments and bounded limits; profiles cannot grant authority.
- [ ] Existing direct and daemon execution paths can consume a validated profile without shell
      interpolation or compatibility regressions.
- [ ] Operators can start, inspect, cooperatively stop, and boundedly restart the daemon with
      deterministic readiness and stale-instance diagnostics.
- [ ] Crash, disconnect, timeout, and ambiguous metadata recovery remains fail-closed and durable.
- [ ] Documentation explains Windows support, profile setup, daemon supervision, and recovery.
- [ ] Full local gate and exact-head CI pass for implementation and closure checkpoints.

## Completion record

Implementation commits:
CI run:
Closure commit:
Closure CI:
Completed:
Notes:
