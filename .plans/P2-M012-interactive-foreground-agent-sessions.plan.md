# Plan: P2-M012 — Interactive foreground agent sessions

Status: Complete
Milestone: P2-M012
Created: 2026-09-20

## Goal

Add an explicit foreground execution mode so an operator can interact with a configured coding
agent while it works in the verified task worktree. The mode must preserve AgentForge's existing
preflight, capability, approval, task-state, audit, timeout, and acceptance boundaries while
streaming bounded live evidence through the operator's current terminal.

## Non-goals

- Do not add daemon attach/detach or terminal-emulator launching in this increment.
- Do not change `forge daemon run`; daemon execution remains background, bounded, and captured.
- Do not build a raw-terminal TUI, full-screen terminal application, terminal-size negotiation, or
  a provider-specific interaction protocol.
- Do not bypass task approvals, capability checks, worktree ownership, audit persistence, or
  independent task acceptance.
- Do not introduce shell interpolation, ambient executable lookup, or a second task prompt source.
- Do not add an external PTY dependency without a separately approved plan amendment. The first
  mode is ordinary cooked, line-oriented terminal interaction and will document that limitation.

## Context

The existing `ProcessAdapter` sends a rendered task prompt through a piped child stdin and captures
stdout/stderr into bounded evidence. `forge run` therefore waits for a result but does not provide
live operator conversation. `forge daemon run` intentionally executes through the detached daemon
and returns a bounded status message. P2-M011 completed the guided intake surface, so the next
operator-facing gap is direct, supervised interaction with the agent during a foreground build.

## Architecture placement

Keep the default captured `ProcessAdapter` behavior unchanged for daemon and non-interactive CLI
runs. Add an explicit interactive execution mode in `agentforge-adapter` that reuses the same
request preflight and worktree verification, writes the initial rendered task prompt, forwards
cooked line-oriented operator input, tees child stdout/stderr to the current terminal, and retains
bounded evidence for the existing orchestrator/audit path. The CLI selects this mode only through
an explicit `--interactive` flag. The orchestrator remains unaware of terminal details, and the
daemon cannot select the mode.

## Data flow

1. `forge run <root> <task-id> <absolute-executable> [--interactive]` or the equivalent profile
   form parses the flag without changing executable or profile validation.
2. Existing approval, task, repository, and managed-worktree preflight completes before any child
   process starts.
3. The interactive adapter launches the direct executable in the verified worktree with the same
   literal arguments and explicit environment contract as the captured adapter, sends the rendered
   task prompt, and opens a cooked line bridge from the operator's terminal to the child.
4. Child stdout/stderr are flushed to the operator as they arrive and simultaneously retained only
   within the configured evidence bound. Timeout and output-limit supervision remain enforced.
5. On child exit, EOF, timeout, or limit termination, the bridge closes, the existing orchestrator
   persists task/audit evidence, and the CLI reports termination without accepting the task.

## Invariants

- Interactive and captured modes perform identical preflight, authority, worktree, and audit
  transitions.
- `--interactive` is opt-in; existing commands, profiles, daemon behavior, and output contracts
  remain compatible when it is absent.
- Input and executable arguments are passed directly; no shell is started or interpolated.
- The operator can provide additional line-oriented answers after the initial task prompt, but input
  cannot grant capabilities, approvals, paths, gates, or acceptance authority.
- Live output is best-effort terminal presentation; durable evidence remains bounded and explicit.
- Interactive termination, child failure, timeout, output-limit termination, and terminal EOF all
  remain distinguishable in the execution report and audit trail.
- A child process is never orphaned by normal command completion, timeout, or operator EOF.

## ADRs

- ADR-0021/0022 remain authoritative for bounded, cooked-mode operator presentation and the
  prohibition on raw terminal control in the current product line.
- Add ADR-0024 to record the foreground interactive-agent decision, its cooked-mode limitation,
  and the explicit separation from daemon attachment.

## Public API / CLI

Extend the direct command with:

```text
forge run <root> <task-id> <absolute-executable> [--interactive]
forge run <root> <task-id> --profile <profile-id> [--interactive]
```

The flag is accepted at most once and is rejected for `forge daemon run`. Without it, the current
bounded captured behavior is unchanged. Interactive output keeps the child streams distinguishable
and ends with the existing launch/termination summary.

## Compatibility analysis

The Rust 1.85 standard-library-only baseline and all persisted task, audit, profile, and worktree
formats remain unchanged. Existing adapter reports and daemon protocol records remain compatible.
Cooked line forwarding works in ordinary terminals and redirected test environments; full-screen or
raw-mode agents are explicitly unsupported until a separate PTY design is approved.

## Dependency analysis

No new dependencies. Use `std::process`, bounded reader/writer threads, synchronization already
used by the adapter, and ordinary line-oriented standard I/O. If cross-platform cleanup proves
impossible without PTY-specific APIs, stop and amend the plan rather than weakening lifecycle
guarantees.

## Expected file boundary

Implementation may modify only:

- `.plans/P2-M012-interactive-foreground-agent-sessions.plan.md` (approval/completion metadata);
- `crates/agentforge-adapter/src/lib.rs`;
- `crates/agentforge-adapter/tests/process_adapter.rs` and the deterministic adapter fixture only
  as needed for interactive coverage;
- `crates/agentforge-cli/src/main.rs`;
- focused CLI integration tests for direct interactive runs;
- `docs/adr/ADR-0024-interactive-agent-sessions.md`;
- `docs/ORCHESTRATION.md`, `docs/AGENT_PROFILES.md`, and focused README command examples;
- `docs/MILESTONES.md`, `PROJECT_STATE.md`, and `AGENT_HANDOFF.md` only during closure evidence.

No daemon, scheduler, worktree, audit format, policy, intake, release, or unrelated workspace files
may change.

## Test-first matrix

| Case | Expected evidence |
| --- | --- |
| Existing captured adapter path | Current bounded adapter tests remain unchanged and green. |
| Interactive direct executable | Initial prompt and subsequent cooked input reach the fixture; live stdout/stderr are visible and bounded evidence is retained. |
| Interactive profile path | Profile arguments/environment remain literal and equivalent to captured mode. |
| Preflight and approval failure | No child starts and no terminal bridge is opened. |
| Operator EOF | Child is closed cooperatively, state/audit evidence is persisted, and no process remains. |
| Timeout and output limit | Child is terminated within existing bounds and termination is reported distinctly. |
| Daemon compatibility | `forge daemon run` remains captured and rejects no existing valid invocation. |
| Linux/macOS/Windows smoke | Direct interactive fixtures complete without shell syntax or platform-specific assumptions. |

## Implementation sequence

1. Finalize the flag grammar, cooked-mode interaction contract, termination behavior, and ADR in
   focused tests/docs before touching process supervision.
2. Add an explicit adapter execution mode that shares preflight and bounded evidence code with the
   existing captured path while keeping the default unchanged.
3. Wire CLI parsing and direct-run construction; reject interactive selection on daemon commands.
4. Extend deterministic fixtures and integration tests for prompt forwarding, live output, EOF,
   timeout, output limits, profile parity, and cross-platform behavior.
5. Run formatting, focused tests, the full local gate, exact implementation CI, and the normal
   closure evidence sequence.

## Failure modes

- Invalid flag placement or duplicate flag: bounded usage error before project mutation.
- Any preflight, approval, worktree, or policy failure: no child or terminal bridge starts.
- Terminal input EOF: close child input cooperatively, preserve evidence, and return a controlled
  termination result.
- Child timeout, output limit, spawn failure, or nonzero exit: preserve bounded live/evidence output,
  persist the existing failure/audit transition, and never mark the task accepted.
- Terminal bridge cleanup cannot be proven on a supported platform: classify as infrastructure or
  design failure and amend the plan before implementation expands.

## Documentation impact

Document the interactive direct-run examples, cooked line-oriented limitation, live-output/evidence
distinction, EOF/timeout behavior, profile usage, and the fact that daemon runs remain detached.
Record the exact implementation and closure CI evidence in the normal milestone records.

## Quality gates

- `cargo fmt --all -- --check`.
- `CARGO_TARGET_DIR=/tmp/agentforge-cargo-target ./scripts/gate.sh full`.
- Focused adapter and CLI integration tests plus the complete workspace suite.
- `git diff --check` and repository policy checks.
- Exact-commit remote CI green across policy, stable, MSRV, CLI smoke, Ubuntu, macOS, and Windows.

## Acceptance criteria

- [x] An explicit direct `forge run --interactive` mode provides live cooked terminal interaction.
- [x] Existing captured and daemon execution behavior remains compatible and bounded.
- [x] Preflight, capability, approval, worktree, audit, timeout, and acceptance boundaries are
      unchanged.
- [x] EOF, child failure, timeout, output limit, and cleanup behavior are tested on supported hosts.
- [x] Documentation clearly distinguishes interactive foreground runs from detached daemon runs.
- [x] Local gate, exact implementation CI, closure commit, and exact closure CI are recorded.

## Completion record

Implementation commit: `4fa5c0d2c00887b5de70e6bcf43ed752e8202936`.
CI run: `35530232141`.
CI result: Green across repository policy, stable, MSRV, CLI smoke, Ubuntu, macOS, and Windows.
Completed: 2026-09-20.
Notes: Added opt-in direct `forge run --interactive` cooked line forwarding with live stdout/stderr
teeing and bounded evidence. Captured and daemon modes remain unchanged; raw PTY/full-screen
interaction and daemon attachment remain future work.
