# ADR-0045: Post-review approvals are bound to the reviewed commit

- Status: Accepted
- Date: 2026-09-24
- Milestone: P1-M008
- Amends: ADR-0026 (safe review and integration)

## Context

Every approval a task declared was required before its agent could run. For
`merge_protected_branch` that meant the operator approved the merge before the agent had written
anything (dogfooding finding 8, P2-M033). The audit record of the approval therefore did not show
that a review happened, and it did not name a commit: a task branch could gain commits after review
and still be integrated.

Phase 4 remote workers will return results produced on other hosts. Accepting those results needs
an approval tied to an exact commit.

## Decision

- Approval boundaries are split by what they authorize. **Post-execution** boundaries act on a
  task's result: `merge_protected_branch`, `publish_release`, and `deploy_production`
  (`ApprovalBoundary::is_post_execution`). All other boundaries are **pre-execution** and gate the
  work itself.
- Launching or running an agent requires only the pre-execution approvals. This applies on every
  path: `forge run`, `task launch`, `launch-batch`, and the daemon's `run` and `launch`.
- A post-execution approval can be recorded only for a `succeeded` (accepted) task with a managed
  worktree. The `ApprovalRecorded` event carries `source_head`, the task branch head at approval
  time. Repeating it for the same head records nothing; approving after the head changed appends a
  new record.
- `forge task integrate` fast-forwards only a head named by a merge approval. The head is checked
  again under the integration lock (`WorktreeManager::integrate_expecting`). A branch that moved
  after approval is refused with both SHAs.
- Approvals recorded before this decision carry no `source_head`. They stay in the audit log, but
  they cannot authorize integration; the operator reviews and approves again.

## Consequences

Positive:

- the operator order is review → accept → approve → integrate, and the audit log proves it;
- an approval names exactly what was reviewed, which later milestones can reuse for remote results,
  releases, and deployments;
- agents no longer need merge authority approved in advance to start.

Negative:

- operators used to approving everything at task creation get a refusal. The message gives the
  order to follow;
- accepted tasks holding a legacy merge approval must be approved again.

## Alternatives considered

- **Record approvals early but re-check at integration.** Rejected: the record still would not
  show a review, and nothing would bind it to a commit.
- **Split approvals into two per-task lists.** Rejected: the boundary itself decides when it
  applies, so a per-task choice would add configuration without adding safety.
