# Plan: P2-M001 — Read-only operator HUD

Status: Completed
Milestone: P2-M001
Created: 2026-09-20

## Goal

Give operators a deterministic terminal dashboard for the current project blueprint, guidelines,
durable task state, audit evidence, and managed worktrees without introducing hidden UI state or
side effects. This is the first usable HUD surface; interactive editing and live refresh remain
later increments.

## Non-goals

- No dependency on a terminal UI framework, daemon, remote service, cloud API, or graphics stack.
- No mutation, task creation, approval, agent launch, worktree lifecycle operation, gate execution,
  merge, deployment, or automatic repair from the HUD.
- No ANSI-control-sequence requirement, terminal-size negotiation, watch loop, or interactive key
  handling in this milestone.
- No replacement of the durable blueprint, task snapshot, audit log, policy, or worktree contracts.
- No new third-party Rust dependency without an explicit plan amendment and approval.

## Context

P1-M003 created durable project intake and explicit task creation, but operators still need one
place to understand project intent and runtime state. The HUD must be a read-only projection of
durable artifacts so a future TUI can add editing without creating a second source of truth.

## Architecture placement

```text
forge hud <root>
       |
       v
read-only HUD snapshot/renderer
       |
       +--> agentforge-intake (blueprint/guidelines)
       +--> agentforge-state (task graph)
       +--> agentforge-audit (verified events)
       +--> agentforge-worktree (verified managed worktrees)
```

Use a focused `agentforge-hud` crate with standard-library-only deterministic models and rendering.
The CLI owns process exit behavior; the HUD crate owns no process spawning and no terminal control.

## Data flow

1. `forge hud <root>` reads the blueprint and guideline documents through the intake boundary.
2. It loads the task snapshot if present and reports missing state explicitly rather than creating
   it.
3. It opens the audit log read-only and summarizes verified record counts, latest sequence, and
   task-linked activity without exposing unbounded raw payloads.
4. It inspects managed worktrees through `WorktreeManager::list`, preserving dirty and unresolved
   operation evidence.
5. It renders a stable plain-text report with project identity, mission, task counts by lifecycle,
   blockers, worktree health, and recent audit activity.

## Invariants

- Rendering is deterministic for equivalent source state and independent of terminal dimensions.
- The command performs no filesystem writes, Git mutations, process launches, network access, or
  approval changes.
- Missing or corrupt individual sources produce explicit bounded diagnostics and a non-success exit
  status; the HUD never guesses or silently treats missing state as healthy.
- Audit and task data remain bounded by existing store limits and a fixed recent-event display cap.
- Worktree status is authoritative only when verified by the existing manager.
- User-controlled paths are passed as direct path arguments; no shell interpolation is used.

## ADRs

- Add `docs/adr/ADR-0021-read-only-operator-hud.md` and register it in `docs/adr/README.md`.
- Explain why the first HUD is a deterministic read-only projection and why terminal rendering is
  separated from durable state and mutation commands.

## Public API / CLI

Add provider-neutral HUD types:

- `HudSnapshot` with bounded project, task, audit, and worktree summaries;
- explicit health/blocker records with stable ordering;
- a plain-text renderer returning bounded output;
- source-specific diagnostics that preserve failure classification.

Add one stable command:

```text
forge hud <root>
```

The command exits zero only when all required sources are readable and verified. It must not create
`.forge` files as a side effect.

## Compatibility analysis

Existing `forge init`, `forge blueprint validate`, `forge task create`, `forge run`, task snapshots,
audit files, and worktree behavior remain unchanged. The HUD consumes public store APIs only.

## Dependency analysis

Use the Rust standard library and existing workspace crates. Do not add a terminal UI dependency in
P2-M001; a later interactive milestone may amend this decision with explicit approval.

## Expected file boundary

- `.plans/P2-M001-operator-hud.plan.md`, `.plans/ACTIVE`
- `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, `docs/MILESTONES.md`
- `docs/HUD.md`, `docs/ORCHESTRATION.md`
- `docs/adr/ADR-0021-read-only-operator-hud.md`, `docs/adr/README.md`
- `Cargo.toml`, `Cargo.lock`
- `crates/agentforge-hud/**`
- `crates/agentforge-cli/**`
- focused unit and integration tests for snapshot collection, bounded rendering, diagnostics, and
  read-only behavior

No interactive TUI, daemon, scheduler, adapter, policy, persistence-format, workflow, hook, or
deployment changes without an amendment.

## Test-first matrix

| Behavior | Required evidence |
| --- | --- |
| Blueprint projection | Name, mission, and guideline version render deterministically |
| Task projection | Pending/running/succeeded/failed/blocked/cancelled counts are stable |
| Audit projection | Verified record count, latest sequence, and recent task activity are bounded |
| Worktree projection | Clean, dirty, unresolved, and missing managed worktrees are distinguished |
| Diagnostics | Missing/corrupt blueprint, snapshot, audit, or Git state fails closed with source context |
| Rendering | Equivalent snapshots produce byte-identical plain text with stable ordering |
| Read-only | HUD execution does not create, modify, or remove project files or worktrees |
| CLI | Success and source-failure exit behavior are deterministic |

## Implementation sequence

1. Approve this plan in a separate plan-only checkpoint and activate it only after the plan pointer
   is present in `HEAD`.
2. Define bounded snapshot fields, source diagnostics, and plain-text layout in the ADR and
   `docs/HUD.md`.
3. Implement snapshot collection and renderer tests before CLI wiring.
4. Add the read-only `forge hud` command and isolated temporary-repository fixture.
5. Run the full local gate and exact-head CI.
6. Close, merge, and verify post-merge CI with exact commit evidence.

## Failure modes

- Missing blueprint or guidelines: report the intake path and stop with failure.
- Missing task snapshot: report no durable task state rather than creating an empty snapshot.
- Corrupt audit log: report audit verification failure and stop with failure.
- Git/worktree inspection error: report the verified Git boundary failure and stop with failure.
- Oversized or unexpectedly large display data: retain bounded summaries and report truncation.

## Documentation impact

Document the HUD report sections, source-of-truth boundaries, failure behavior, and the fact that
the first HUD is a snapshot command rather than an interactive editor.

## Quality gates

- `./scripts/gate.sh full`
- focused HUD and CLI tests
- MSRV 1.85.0 compatibility
- exact-head Repository policy, Stable code gate, MSRV, and CLI smoke CI
- no bypassed hooks or weakened validation

## Acceptance criteria

- [x] An operator can run `forge hud <root>` and see a deterministic project/task/audit/worktree snapshot.
- [x] HUD output is read-only, bounded, plain text, and stable across equivalent inputs.
- [x] Missing or corrupt sources fail closed with actionable diagnostics.
- [x] Tests prove source projection, bounded rendering, diagnostics, and no mutation.
- [x] Exact implementation, closure, merge, and post-merge CI evidence is recorded.

## Completion record

Implementation commit: `ca741baeda2fd040e6df714f9d2c3af18d16ba76`
CI run: `35496675009`
CI result: all four jobs green
Completed: 2026-09-20
Notes: Added the dependency-free HUD snapshot/renderer, `forge hud <root>`, source-failure
diagnostics, bounded deterministic output, ADR-0021, and read-only CLI/integration coverage.
