# ADR-0010: Deterministic managed worktree lifecycle

- Status: Accepted
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

ADR-0002 selects Git worktrees as the isolation boundary, but safe orchestration also requires a
stable ownership convention and conservative cleanup rules.

Provider-specific agents must not choose arbitrary branches, paths, or destructive cleanup
behavior.

## Decision

Each validated AgentForge task deterministically maps to one managed branch and one managed
worktree path.

Default identities:

- branch: `agentforge/task/<task-id>`;
- worktree: `<project-root>/.forge/worktrees/<task-id>`.

Git's worktree registry is authoritative for confirming that a physical path is actually a linked
worktree.

Creation resolves the requested base ref to an exact commit.

Normal retirement requires a clean worktree with no unresolved Git operation, uses non-forced
`git worktree remove`, and preserves the task branch.

Git commands are executed directly with argument arrays rather than through a shell.

## Consequences

Worktree ownership is deterministic and auditable.

Agent adapters can receive isolated workspaces without gaining authority to invent cleanup policy.

Dirty or abandoned work remains available for inspection instead of being destroyed automatically.

Future integration logic may separately decide when a preserved task branch is safe to delete.
