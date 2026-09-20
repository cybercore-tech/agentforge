# ADR-0015: Explicit least-privilege capability policy

- Status: Proposed
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

Roles identify responsibility but cannot safely determine authority. Tasks already carry explicit
capabilities, owned paths, and approval boundaries; callers need one deterministic validation point.

## Proposed decision

Introduce a provider-neutral `agentforge-policy` engine that evaluates task grants and requested
operations with deny-by-default semantics. Capabilities remain independent of roles. The engine
returns deterministic allow, denial, or escalation evidence and never grants authority, consults
ambient environment, launches processes, or claims to enforce OS security boundaries.

## Consequences

Policy checks become reusable and testable before adapters or orchestration side effects. Callers
remain responsible for obtaining approvals and enforcing decisions at execution boundaries.

## References

- `docs/GOVERNANCE.md`
- `docs/APPROVAL_BOUNDARIES.md`
- `.plans/P0-M010-capability-policy.plan.md`
