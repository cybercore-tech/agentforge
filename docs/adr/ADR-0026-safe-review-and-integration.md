# ADR-0026: Safe review and integration workflow

## Status

Accepted for P2-M014 implementation.

## Decision

AgentForge exposes review and protected-branch integration as two explicit operator actions:

```text
forge task diff <root> <task-id>
forge task integrate <root> <task-id> --target <branch> --actor <actor-id>
```

`task diff` is read-only. It verifies the managed task worktree against the checked-out target,
resolves exact commit IDs, uses Git's merge base, and emits a bounded changed-path projection.
It does not approve, transition, merge, retire, or append audit state.

`task integrate` requires a succeeded task, the `merge_protected_branch` capability, the matching
recorded approval, a clean source and target, no unresolved Git operation, and a checked-out target
branch that exactly matches the explicit argument. It serializes integrations with a create-new
`.forge/integration.lock`, verifies ancestry, executes only literal `git merge --ff-only`, verifies
the target head, and records an integrity-linked `IntegrationRecorded` audit event. It never force-
removes a worktree, deletes a task branch, auto-accepts a task, or steals a lock.

Repeated integration is idempotent: once the target already contains the exact source head, the
operation reports `already_integrated` and does not append duplicate integration evidence.

## Consequences

Review remains inspectable and safe to repeat. Protected integration is deliberately narrower than
general Git usage, so detached targets, dirty state, ambiguous ownership, stale source heads,
non-fast-forward histories, unresolved operations, and concurrent attempts fail closed.
