# Plan: P2-M013 — PTY-backed foreground agent sessions

Status: Draft
Milestone: P2-M013
Created: 2026-09-20

## Goal

Add an explicit pseudo-terminal (PTY) execution mode for direct foreground runs so terminal-native
agents such as Codex can receive a real controlling terminal, render cursor/full-screen interfaces,
and accept byte-oriented operator input while remaining inside AgentForge's existing supervision and
authority boundaries.

## Context

P2-M012 added `forge run --interactive` with a cooked, line-oriented stdin/stdout bridge. Real
operator dogfooding showed that a full-screen agent can remain alive until timeout because a pipe is
not a terminal: cursor movement, raw key input, terminal size, and terminal control sequences are
not available. P2-M013 addresses that concrete gap without silently changing the current mode or
daemon execution.

## Non-goals

- Do not change the default captured adapter behavior.
- Do not change cooked `--interactive` semantics; it remains available for pipe-friendly agents.
- Do not allow PTY selection for `forge daemon run`; daemon execution remains detached and captured.
- Do not add provider-specific logic for Codex, Claude, or any other agent.
- Do not add shell interpolation, ambient executable lookup, or a second task-prompt source.
- Do not weaken capability, approval, policy, worktree, audit, timeout, output, or independent
  acceptance boundaries.
- Do not support terminal multiplexing, daemon attach/detach, remote terminals, mouse protocols, or
  terminal recording beyond the existing bounded evidence contract.

## Proposed user-facing contract

Keep the existing modes distinct and make PTY behavior explicit:

```text
forge run <root> <task-id> <absolute-executable> --interactive --pty
forge run <root> <task-id> --profile <profile-id> --interactive --pty
```

`--pty` requires `--interactive`, is accepted at most once, and is rejected by daemon commands.
Without `--pty`, all existing captured and cooked-interactive behavior remains unchanged. PTY mode
requires an interactive operator stdin/stdout; it fails closed before child spawn when either stream
is not a terminal.

## Architecture

Use a small adapter-side terminal session abstraction backed by a cross-platform PTY implementation
and a terminal-mode guard:

- `portable-pty` `0.9.x` provides Unix PTY and Windows ConPTY creation, direct executable/argument
  spawning, child handles, master reader/writer, and initial window sizing.
- `crossterm` `0.29.x` provides cross-platform raw-mode enable/restore and terminal-size detection;
  raw mode is entered only after all preflight checks and is restored by an RAII guard on every exit
  path, including spawn failure, timeout, output-limit termination, EOF, panic, and signal-like
  cleanup paths available to the supported platforms.
- The PTY child inherits only the explicit cleared environment, verified worktree cwd, and literal
  profile arguments already used by the captured adapter. No shell is introduced.
- The rendered provider-neutral task prompt is written first to the PTY master. Subsequent operator
  bytes flow bidirectionally until terminal EOF, child exit, timeout, or output-limit termination.
- PTY output is copied live to the operator terminal and retained under the existing bounded stdout/
  stderr evidence limit. PTY output is classified as terminal evidence; durable task/audit state
  remains authoritative.

The implementation must prove child cleanup and PTY/master closure before returning. If a supported
platform cannot provide those guarantees with the selected dependency, stop and amend this plan
rather than falling back silently to a pipe or forced process termination.

## Dependency and compatibility analysis

This increment intentionally introduces the first terminal dependencies:

- `portable-pty = "0.9"` in the adapter crate;
- `crossterm = "0.29"` with only the terminal/event capabilities needed for raw mode and size
  detection;
- corresponding `Cargo.lock` updates.

Validate both dependencies against the Rust 1.85 MSRV, license policy, supported Linux/macOS/
Windows matrix, and offline/reproducible lockfile behavior before implementation. No other
dependency changes are authorized by this plan. Existing captured and cooked paths must compile
and test when PTY code is unavailable or the process is launched without a terminal.

## Safety invariants

- All existing preflight checks complete before opening raw operator mode or spawning the PTY child.
- PTY mode cannot grant capabilities, approvals, paths, gates, or acceptance authority.
- Terminal raw mode is never left enabled after normal completion, timeout, EOF, child failure, or
  any handled error.
- Initial terminal dimensions are bounded and passed to the PTY; unsupported resize notification is
  reported explicitly rather than guessed. Dynamic resize forwarding may be included only if it is
  deterministic and tested on all supported hosts.
- Child stdout/stderr and operator input remain bounded; output-limit and timeout supervision still
  terminate the child and close the PTY without orphaning it.
- Non-TTY stdin/stdout, malformed flags, duplicate flags, invalid profiles, and preflight failures
  fail before child spawn and before raw-mode activation.
- Daemon runs and existing captured/cooked reports retain their current termination and evidence
  contracts.

## Expected file boundary

Implementation may modify only:

- `.plans/P2-M013-pty-foreground-agent-sessions.plan.md` (approval/completion metadata);
- `Cargo.toml` and `Cargo.lock` for the explicitly approved terminal dependencies;
- `crates/agentforge-adapter/Cargo.toml`;
- `crates/agentforge-adapter/src/lib.rs` and focused adapter PTY/session modules if extraction is
  needed;
- `crates/agentforge-adapter/tests/process_adapter.rs` and deterministic terminal fixtures;
- `crates/agentforge-cli/src/main.rs`;
- focused CLI tests for PTY flag parsing, non-TTY rejection, and direct profile execution;
- `docs/adr/ADR-0025-pty-foreground-agent-sessions.md`;
- `docs/ORCHESTRATION.md`, `docs/AGENT_PROFILES.md`, and focused README command examples;
- `docs/MILESTONES.md`, `PROJECT_STATE.md`, and `AGENT_HANDOFF.md` only during closure evidence.

No daemon protocol, scheduler, worktree, audit format, intake, release workflow, or unrelated
workspace files may change.

## Test-first matrix

| Case | Expected evidence |
| --- | --- |
| Existing captured adapter | Current captured tests and reports remain unchanged. |
| Existing cooked interactive adapter | Current line-oriented fixture and CLI test remain green. |
| PTY adapter fixture | Child observes a terminal, receives the rendered prompt and byte input, and emits cursor/control sequences that reach the operator stream. |
| PTY profile execution | Literal executable, arguments, environment, cwd, and initial dimensions are preserved. |
| Non-TTY invocation | `--pty` fails with a clear diagnostic before child spawn and without raw-mode mutation. |
| Preflight/approval failure | No PTY, raw mode, or child process is opened. |
| EOF and child exit | PTY closes cooperatively, raw mode restores, bounded evidence persists, and no child remains. |
| Timeout and output limit | Child and PTY are terminated within bounds, termination is distinct, and terminal state restores. |
| Spawn/PTY setup failure | Controlled error with no leaked raw mode or orphaned process. |
| Flag compatibility | Duplicate/standalone `--pty` and daemon `--pty` are rejected deterministically. |
| Linux/macOS/Windows smoke | PTY code compiles and supported fixture tests pass on the exact CI matrix; unsupported host behavior is explicit. |

## Implementation sequence

1. Validate dependency versions, licensing, MSRV, and platform APIs; record any incompatibility
   before source changes.
2. Add a terminal-session abstraction and RAII raw-mode guard without changing existing modes.
3. Implement direct PTY spawning, bounded bidirectional I/O, initial sizing, timeout/output-limit
   supervision, child cleanup, and structured termination evidence.
4. Wire `--pty` through direct CLI/profile parsing and reject invalid or daemon combinations.
5. Extend deterministic fixtures and tests, including non-TTY fail-closed coverage and terminal
   restoration checks.
6. Update ADRs and operator documentation with Codex-compatible examples, cooked-vs-PTY behavior,
   cleanup guarantees, and unsupported terminal features.
7. Run focused tests, the full local gate, exact implementation CI, and the normal closure evidence
   sequence.

## Failure classification

- Non-TTY or invalid flag: controlled usage/preflight failure; no mutation beyond existing audit
  rules.
- PTY creation, raw-mode, resize, or cleanup incompatibility: infrastructure/design failure; stop
  and amend the plan rather than weakening guarantees.
- Child timeout, output limit, EOF, spawn failure, or nonzero exit: preserve bounded evidence and
  distinguish termination without accepting the task.
- Dependency/MSRV/platform failure: dependency/toolchain classification; repair only that category.

## Documentation impact

Document that `--interactive --pty` is the terminal-native foreground mode for full-screen agents,
while `--interactive` alone remains cooked line-oriented and captured/daemon modes remain bounded.
Explain that PTY mode requires a real terminal, preserves task authority boundaries, restores
terminal state, and does not create a daemon attachment or release guarantee.

## Quality gates

- `cargo fmt --all -- --check`.
- `CARGO_TARGET_DIR=/tmp/agentforge-cargo-target ./scripts/gate.sh full`.
- Focused adapter and CLI PTY tests plus the complete workspace suite.
- `git diff --check` and repository policy checks.
- Exact-commit remote CI green across repository policy, stable, MSRV, CLI smoke, Ubuntu, macOS,
  and Windows.

## Acceptance criteria

- [ ] A distinct `--interactive --pty` direct-run mode gives terminal-native agents a real PTY.
- [ ] Existing captured, cooked-interactive, and daemon behavior remains compatible and bounded.
- [ ] TTY detection, raw-mode restoration, EOF, timeout, output limits, child cleanup, and spawn
      failures are fail-closed and tested.
- [ ] Literal arguments/environment, verified worktree cwd, prompt ordering, and authority checks
      remain unchanged.
- [ ] Linux, macOS, and Windows builds/tests cover the supported PTY path or clearly report an
      unsupported terminal condition.
- [ ] Documentation distinguishes captured, cooked, and PTY foreground modes.
- [ ] Local gate, exact implementation CI, closure commit, and exact closure CI are recorded.

## Completion record

Implementation commit: pending.
Exact implementation CI: pending.
Closure commit: pending.
Exact closure CI: pending.
