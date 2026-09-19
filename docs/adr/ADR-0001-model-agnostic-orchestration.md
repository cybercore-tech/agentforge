# ADR-0001: Model-agnostic orchestration

- Status: Accepted
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

Coding models and agent products change rapidly. AgentForge must not couple durable project
workflow to one provider.

## Decision

AgentForge defines its own task, capability, gate, result, and handoff contracts. External agents
integrate through adapters.

## Consequences

Models can be replaced without rewriting project state or workflow. Provider-specific features may
be used behind adapters but must not become required project truth.
