# ADR-0022: Cooked-mode interactive HUD watch

Status: Accepted

## Decision

P2-M002 extends the read-only HUD with an optional bounded watch mode:
`forge hud <root> --watch [--interval-ms N]`. It uses ordinary cooked-mode, line-oriented input
(`r`/`refresh`, `h`/`help`, and `q`/`quit`) and keeps source collection and deterministic rendering
inside the existing HUD boundary.

The watch loop owns only transient memory. Each frame is a fresh verified projection of the durable
intake, task, audit, and worktree sources. Source failures are displayed explicitly for that frame
and may recover on a later refresh; no failure is treated as healthy state.

## Rationale

Cooked-mode input works in ordinary terminals and redirected test environments without unsafe
terminal FFI, raw-mode signal handling, terminal-size negotiation, or a TUI dependency. Polling is
bounded to prevent accidental busy loops while preserving the existing one-shot command and plain
text output contract.

## Consequences

The HUD remains an observation surface, not an orchestration control surface. It cannot edit
project files, change task state, launch agents, mutate Git, or approve work. A future richer TUI
may add presentation affordances only through a separately approved plan.
