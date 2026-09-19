# ADR-0002: Git worktrees are the task isolation boundary

- Status: Accepted
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

Concurrent agents editing one working tree produce conflicts, hidden state, and unreliable recovery.

## Decision

Implementation tasks use isolated Git branches and worktrees. Agents do not share a dirty working
tree.

## Consequences

Parallel work is easier to audit, abandon, review, and merge independently. Worktree lifecycle
management becomes a core AgentForge responsibility.
