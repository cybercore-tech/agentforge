# AgentForge Governance

AgentForge uses human-controlled, least-privilege agent execution.

## Governing principles

1. Models are replaceable workers.
2. Roles describe responsibility.
3. Capabilities describe authority.
4. Tasks describe scope.
5. Plans describe intended change.
6. Gates provide evidence.
7. Humans approve consequential boundaries.
8. Results report what happened but cannot self-authorize future actions.

## Reviewer independence

Reviewers are read-oriented by default.

When a reviewer discovers a defect, the normal output is a finding or repair task. A reviewer does
not silently transform itself into the implementer whose work it is reviewing.

## Concurrent work

Write-capable tasks receive non-overlapping ownership by default.

Shared generated files or integration surfaces require serialization or an explicitly assigned
Integrator.

Read-only inspection may occur concurrently.

Dirty working trees are never shared between agents.

## Escalation

An agent must stop and request escalation when:

- required work exceeds its allowed paths;
- required capability was not granted;
- a human approval boundary is reached;
- a destructive or irreversible operation becomes necessary;
- project state conflicts with the task contract;
- evidence is insufficient to make a safe decision.

Escalation is a valid task outcome, not an agent failure.
