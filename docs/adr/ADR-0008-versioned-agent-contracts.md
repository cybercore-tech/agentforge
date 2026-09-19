# ADR-0008: Versioned structured task and result contracts

- Status: Accepted
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

Agent workers need durable, auditable inputs and outputs that survive model replacement.

Free-form prompts alone are not sufficient project state.

## Decision

AgentForge uses versioned semantic contracts for task input and execution result output.

The semantic model is provider-neutral.

P0-M002 does not stabilize a serialization encoding.

## Consequences

Future adapters translate a stable AgentForge task model into provider-specific prompts or APIs.

Results can be validated before they affect project state.

Contract evolution is explicit rather than hidden inside prompts.

## References

- `docs/TASK_CONTRACT.md`
- `PROJECT_SPEC.md`
