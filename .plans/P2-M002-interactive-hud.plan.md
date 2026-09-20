# Plan: P2-M002 — Interactive operator HUD

Status: Completed
Milestone: P2-M002
Created: 2026-09-20

## Goal

Extend the completed P2-M001 snapshot into an operator-friendly live HUD. An operator should be
able to launch a bounded refresh loop, inspect the latest durable project/task/audit/worktree
state, refresh on demand, and exit cleanly without introducing a second source of truth or any
mutation authority.

## Non-goals

- No task creation, approval, agent launch, gate execution, worktree lifecycle operation, merge,
  deployment, or automatic repair from the HUD.
- No editing of blueprint, guidelines, task state, policy, or audit records.
- No daemon, cloud service, network access, terminal UI framework, or new third-party dependency.
- No raw-terminal mode, unsafe terminal FFI, terminal-size negotiation, mouse support, or complex
  widget/layout system in this increment.
- No replacement of the existing `forge hud <root>` one-shot command or durable source contracts.

## Context

P2-M001 provides a deterministic one-shot report through `forge hud <root>`. Operators still need
to repeatedly inspect changing task and worktree state during a run. A line-oriented live mode is
the smallest useful interactive increment: it preserves the existing renderer, works in ordinary
terminals and redirected output, and avoids hidden state or terminal-specific mutation.

## Architecture placement

```text
forge hud <root> --watch [--interval-ms N]
       |
       v
line-oriented watch controller
       |
       +--> bounded HUD snapshot provider
       +--> existing deterministic renderer
       +--> stdin command reader (refresh/help/quit)
```

Keep source collection and rendering in `agentforge-hud`. Keep argument parsing, process exit
codes, and terminal I/O policy in `agentforge-cli`. The watch controller owns only transient
in-memory state and never writes project files.

## Data flow

1. Parse `forge hud <root> --watch [--interval-ms N]` with a bounded interval.
2. Collect and render a fresh snapshot immediately using the P2-M001 source boundaries.
3. Wait for either the interval deadline or one cooked-mode stdin command.
4. `r`/`refresh` collects and renders immediately; `h`/`help` prints the bounded command help;
   `q`/`quit` exits successfully.
5. On a source failure, render a source-labelled diagnostic for that cycle and retain the loop;
   a later refresh may recover. Invalid arguments and an unreadable root remain non-success exits.
6. On EOF or terminal interruption, exit without changing durable state.

## Invariants

- One-shot `forge hud <root>` output and exit behavior remain unchanged.
- Every rendered frame is deterministic for its source snapshot and bounded by the existing HUD
  output limit.
- Watch mode performs no filesystem writes, Git mutations, process launches, network access, or
  approval changes.
- Refreshes replace the prior in-memory frame atomically from the operator's perspective; partial
  source results are never presented as healthy state.
- Interval values are explicit, finite, and clamped to documented minimum and maximum bounds.
- Input commands are line-oriented, case-stable, bounded in length, and ignored when empty.
- No shell interpolation or terminal escape sequence is required for correctness.

## ADRs

- Add `docs/adr/ADR-0022-interactive-hud-watch-mode.md` and register it in `docs/adr/README.md`.
- Explain why cooked-mode line commands and bounded polling are preferred over raw-terminal FFI or
  a TUI dependency, and why live HUD state remains a projection only.

## Public API / CLI

Add provider-neutral watch types in `agentforge-hud`:

- bounded `WatchConfig` with default/minimum/maximum interval constants;
- a watch command model (`Refresh`, `Help`, `Quit`, `Ignore`);
- a source-provider boundary suitable for deterministic tests;
- bounded frame/report helpers that reuse the P2-M001 renderer.

Extend the CLI with:

```text
forge hud <root> --watch [--interval-ms <milliseconds>]
```

The existing one-shot form remains supported. Watch-mode help must document `r`, `h`, and `q`.

## Compatibility analysis

Existing `forge hud <root>`, intake commands, task state, audit files, worktree behavior, and
orchestration commands remain unchanged. Watch mode reads the same durable artifacts and does not
create a missing snapshot or audit log. Redirected stdout remains valid plain text; terminal
control sequences are optional presentation only and are not required by tests or correctness.

## Dependency analysis

Use the Rust standard library and existing workspace crates only. Do not add a terminal UI,
polling, or signal-handling dependency in P2-M002. If future raw-terminal interaction requires
platform APIs, it must be proposed as a separate amended plan.

## Expected file boundary

- `.plans/P2-M002-interactive-hud.plan.md`, `.plans/ACTIVE`
- `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, `docs/MILESTONES.md`
- `docs/HUD.md`, `docs/ORCHESTRATION.md`
- `docs/adr/ADR-0022-interactive-hud-watch-mode.md`, `docs/adr/README.md`
- `Cargo.toml`, `Cargo.lock` only if workspace metadata changes
- `crates/agentforge-hud/**`
- `crates/agentforge-cli/**`
- focused unit and integration tests for command parsing, interval bounds, refresh behavior, EOF,
  diagnostics, deterministic frames, and read-only operation

No changes to daemon, scheduler, adapter, gate, policy, persistence format, workflow, hooks, or
deployment behavior without an amendment.

## Test-first matrix

| Behavior | Required evidence |
| --- | --- |
| Command parsing | One-shot compatibility, watch flag, interval bounds, unknown options |
| Input handling | `r`, `refresh`, `h`, `help`, `q`, `quit`, empty input, bounded invalid input |
| Refresh loop | Immediate first frame, interval refresh, explicit refresh, EOF exit |
| Failure recovery | A failed source frame is explicit and a later valid frame can recover |
| Rendering | Equivalent frames remain byte-identical and bounded |
| Read-only | Watch execution creates/modifies/removes no project files or worktrees |
| CLI | Invalid arguments and root failures have deterministic non-success behavior |
| Compatibility | Existing P2-M001 HUD and all workspace tests remain green |

## Implementation sequence

1. Approve this plan in a separate plan-only checkpoint and activate it only after the pointer is
   present in `HEAD`.
2. Define the watch configuration, line-command parser, provider boundary, and frame semantics in
   ADR-0022 and `docs/HUD.md`.
3. Add unit tests for parsing, bounds, deterministic frames, and failure recovery.
4. Implement the bounded watch controller and CLI argument/exit behavior.
5. Add isolated CLI tests using scripted stdin and temporary Git repositories.
6. Run the full local gate and exact-head CI.
7. Close the milestone with exact implementation, closure, and mainline CI evidence.

## Failure modes

- Invalid or missing root: fail with a deterministic CLI diagnostic and non-success exit.
- Invalid interval or unknown option: fail before collecting sources.
- Missing/corrupt source during a frame: display bounded source context and continue watch mode.
- Oversized input line: discard the remainder of that line and display a bounded diagnostic.
- EOF or interrupted stdin: exit without mutation.
- Unexpected renderer/provider failure: fail closed for that frame; never guess healthy state.

## Documentation impact

Document watch-mode invocation, interval bounds, line commands, redirected-output behavior,
failure recovery, and the continued read-only/source-of-truth boundary. Explicitly state that
P2-M002 is a live snapshot loop, not an editor or orchestration control surface.

## Quality gates

- `./scripts/gate.sh full`
- focused HUD and CLI watch tests
- MSRV 1.85.0 compatibility
- exact-head Repository policy, Stable code gate, MSRV, and CLI smoke CI
- no bypassed hooks or weakened validation

## Acceptance criteria

- [x] `forge hud <root>` remains backward-compatible and deterministic.
- [x] An operator can run bounded watch mode and refresh or exit with documented line commands.
- [x] Source failures are explicit per frame and can recover on a later refresh.
- [x] Watch output remains bounded, plain text, and deterministic for equivalent frames.
- [x] Tests prove parsing, interval bounds, refresh/EOF behavior, recovery, and no mutation.
- [x] Exact implementation, closure, and mainline CI evidence is recorded.

## Completion record

Implementation commit: `11792769304f8042235146d670eb554b0bc0cde3`
CI run: `35497283972`
CI result: all four jobs green
Completed: 2026-09-20
Notes: Added bounded cooked-mode watch support with interval clamping, refresh/help/quit commands,
recoverable frame diagnostics, read-only CLI coverage, and ADR-0022.
