# ADR-0028: Daemon task-launch parity

- Status: Accepted
- Date: 2026-09-21
- Decision owners: AgentForge project

## Context

P2-M015 added `forge task launch`, which safely composes exact-base resolution, deterministic
managed worktree preparation, persisted execution, and durable worktree observation for a
foreground task. Detached `forge daemon run` still required a separate worktree-create command.
That separation is safe but creates a needless operator failure point and leaves the two primary
execution modes inconsistent at the preparation boundary.

## Decision

Add a versioned daemon launch request and matching CLI command:

```text
forge daemon launch <root> <task-id> <absolute-executable> [--base <ref>]
forge daemon launch <root> <task-id> --profile <profile> [--base <ref>]
```

The daemon delegates launch preparation and persisted execution to the existing orchestrator
authority. It validates task readiness, capabilities, approvals, repository identity, and exact
base resolution before creating or reusing the deterministic managed worktree. It records the
worktree observation before invoking the bounded process adapter. The existing `daemon run`
protocol and prepared-worktree semantics remain compatible.

## Consequences

Detached operators can use one explicit command for safe task preparation and execution while
retaining the daemon's loopback-only, serialized, cooperative lifecycle. Repeated launch is
bounded and idempotent at the worktree boundary; unsafe or unrelated worktrees fail closed.
Process success remains evidence only. Acceptance, review, integration, and retirement remain
independent operator actions, and no forced cleanup or implicit Git mutation is introduced.
