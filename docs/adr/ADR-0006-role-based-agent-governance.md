# ADR-0006: Role-based agent governance

- Status: Accepted
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

AgentForge must assign responsibility without coupling project policy to model vendors.

## Decision

Every agent task has one primary role from a project-defined role registry.

Roles describe responsibility and review expectations, not model identity or authority.

## Consequences

The same provider or model may serve different roles on separate tasks.

Changing models does not change task semantics.

Authority remains governed separately through explicit capabilities.

## References

- `docs/AGENT_ROLES.md`
- `docs/GOVERNANCE.md`
- `PROJECT_SPEC.md`
