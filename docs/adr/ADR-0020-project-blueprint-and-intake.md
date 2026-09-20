# ADR-0020: Durable project blueprint and task intake

## Status

Accepted

## Context

AgentForge can execute a task that already exists in durable state, but operators do not yet have a
supported path to author project intent, guidelines, or explicit task contracts. A future TUI needs
a stable source of truth rather than its own hidden state.

## Decision

Use two project-local, versioned, human-editable files under `.forge/`: a bounded structured
`blueprint.conf` and a bounded Markdown `guidelines.md`. The CLI initializes and validates both
without overwriting existing files. Explicit task creation compiles structured blueprint defaults
and command-line authority into the existing `AgentTask` contract and persists it through
`FileTaskStore`.

Guideline prose is context only. It can never grant capabilities, approvals, paths, or gates. Task
contracts remain the executable authority boundary and continue through the existing policy and
orchestration checks. The initial interface is CLI/editor based; a future TUI will edit and observe
the same durable artifacts.

## Consequences

- Project intent is reviewable in Git and usable without a daemon or cloud service.
- Validation can fail closed with deterministic source locations before task-state mutation.
- The initial format remains dependency-free and compatible with the Rust 1.85 baseline.
- Natural-language decomposition and visual editing remain future work.
