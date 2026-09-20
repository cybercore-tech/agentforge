# ADR-0024: Cooked-mode interactive foreground agent sessions

Status: Accepted

## Decision

`forge run` may opt into a foreground `--interactive` mode. The direct executable still runs in
the verified task worktree behind the normal capability, approval, policy, audit, timeout, and
acceptance boundaries, but its initial task prompt and subsequent operator input travel through a
cooked, line-oriented bridge. Child output is flushed to the current terminal while bounded copies
remain available as execution evidence.

The default captured adapter path and detached `forge daemon run` path do not change. Interactive
mode does not launch a terminal emulator, expose a network endpoint, or provide raw terminal/PTY
semantics. Full-screen and raw-mode agents require a future separately approved PTY design.

## Rationale

Operators need to answer agent questions and observe progress during a foreground build. Inheriting
terminal streams without first delivering the provider-neutral task prompt would break the adapter
contract, while changing every daemon execution into a terminal session would weaken deterministic
supervision. An explicit cooked bridge provides direct interaction without creating a second task
authority or altering background operation.

## Consequences

- `forge run ... --interactive` is a foreground operator action and should be used from a terminal.
- Input is line-oriented; terminal escape sequences, full-screen rendering, and terminal-size
  negotiation are intentionally unsupported.
- Live presentation is not the durable source of truth; bounded execution evidence and audit/state
  transitions remain authoritative.
- Timeout, output limits, child failure, EOF, and acceptance boundaries remain explicit.
