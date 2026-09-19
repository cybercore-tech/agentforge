# ADR-0007: Capabilities are independent of roles

- Status: Accepted
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

A role name is too coarse to determine what an agent may actually do.

For example, an Implementer may need repository writes but not network, secret, GitHub-write, merge,
or deployment authority.

## Decision

AgentForge grants capabilities explicitly and independently from role identity.

Least privilege is the default.

Agents may request escalation but may not grant themselves new capabilities.

## Consequences

Two tasks with the same role may have different authority.

Privilege expansion becomes an explicit policy event.

## References

- `docs/GOVERNANCE.md`
- `docs/APPROVAL_BOUNDARIES.md`
