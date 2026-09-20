# ADR-0025: PTY-backed foreground agent sessions

Status: Accepted

## Decision

`forge run` may opt into an explicit `--interactive --pty` mode. AgentForge allocates a native
pseudo-terminal for the verified task worktree, starts the configured executable with literal
arguments and explicit environment, forwards keyboard events and terminal resize events, and
streams the PTY output to the operator's terminal. A bounded copy of that output remains the
execution evidence used by the existing orchestration and audit path.

PTY mode is direct-foreground only. It requires terminal stdin and stdout, enables raw mode only
for the session, and restores the operator terminal when the session exits or fails. Child
termination, timeout, output limits, worktree preflight, capability checks, approvals, audit
transitions, and independent task acceptance remain unchanged. Captured runs, cooked
`--interactive` runs, and detached `forge daemon run` remain separate modes.

## Rationale

Full-screen agents such as terminal UIs require a controlling pseudo-terminal and raw keyboard
input. The existing captured and cooked bridges intentionally do not provide those semantics.
Making PTY behavior an explicit opt-in preserves the safe default while allowing an operator to
use a real terminal for agents that need terminal negotiation and screen control.

## Consequences

- PTY sessions cannot be launched through pipes, CI capture, or the detached daemon; they fail
  closed before child spawn when stdin or stdout is not a terminal.
- Terminal dimensions are applied at startup and updated on resize events.
- Output evidence remains bounded and is not a transcript or a replacement for durable state.
- Mouse reporting, terminal recording, and daemon attach are outside this decision.
- Providers remain model-agnostic: AgentForge supervises a process and does not add shell
  interpolation or provider-specific command construction.
