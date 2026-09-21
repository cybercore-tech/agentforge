# ADR-0027: Foreground real-project orchestration pilot

- Status: Accepted
- Date: 2026-09-21
- Decision owners: AgentForge project

## Context

The safe direct execution path required operators to create a managed worktree before invoking
`forge run`. That separation is useful for recovery, but it made a real-project pilot needlessly
ceremonial and left the most important composition boundary exercised only by manual command
sequences.

## Decision

Provide one explicit foreground command that composes the existing task, policy, worktree, audit,
and process-execution authorities:

```text
forge task launch <root> <task-id> <absolute-executable> [--base <ref>] [--interactive] [--pty]
forge task launch <root> <task-id> --profile <profile> [--base <ref>] [--interactive] [--pty]
```

The command validates a ready pending task, required capability and approvals, repository identity,
and adapter configuration before creating or inspecting a worktree. It resolves the base ref to an
exact commit, reuses only an owned clean worktree, records a durable worktree observation, and then
delegates to the existing bounded persisted process path.

Successful execution remains evidence, not acceptance. The command never accepts, integrates,
retires, deletes branches, resets, cleans, or adopts an unrelated worktree. Failure and timeout
evidence remain available for explicit inspection and repair-forward actions.

## Consequences

An operator can exercise one real project task with one explicit launch command while retaining the
existing human-controlled review, acceptance, integration, and retirement boundaries. The pilot is
foreground-only; daemon scheduling and multi-task coordination remain separate concerns.
