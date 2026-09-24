# AgentForge Human Approval Boundaries

Human approval is authoritative.

The default repository-wide boundaries are:

- activating an implementation plan;
- expanding task scope beyond approved ownership;
- adding or materially changing third-party dependencies;
- capability or privilege elevation;
- access to secrets;
- destructive data migration;
- irreversible external state changes;
- merging into a protected branch;
- publishing a release;
- production deployment;
- changing the governance rules that define approval boundaries.

Projects may require additional approvals.

An unrelated implementation task must never weaken these boundaries as a side effect.

## When approvals are recorded

Boundaries fall into two groups (ADR-0045):

- **Pre-execution** approvals gate the work itself, so they are recorded before the agent runs:
  plan activation, scope expansion, dependency changes, capability elevation, secrets, destructive
  migrations, irreversible external changes, and governance changes.
- **Post-execution** approvals act on the result: `merge_protected_branch`, `publish_release`, and
  `deploy_production`. They are **not** required to launch or run an agent. They can be recorded
  only after the task is accepted, and each one is bound to the task branch head at that moment
  (`source_head` in the audit log).

Protected integration is an explicit operator action. A task must declare both the
`merge_protected_branch` capability and `merge_protected_branch` approval requirement. The order
is:

```bash
forge task diff . <task-id>                        # review
forge task accept . <task-id> --actor <you>
forge task approve . <task-id> merge_protected_branch --actor <you>   # prints "at <sha>"
forge task integrate . <task-id> --target main --actor <you>
```

`forge task integrate` fast-forwards only the commit the approval names, checked again under the
integration lock. If the branch gained commits after approval, integration is refused with both
SHAs: review the new commits and approve again. Approvals recorded before P1-M008 are not bound to
a commit and cannot authorize integration. The command accepts only a clean, verified
fast-forward target and records the result in the append-only audit log.
