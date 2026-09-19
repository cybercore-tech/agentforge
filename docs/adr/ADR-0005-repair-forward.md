# ADR-0005: Repair failures forward

- Status: Accepted
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

Destructive resets erase evidence and can destroy unrelated work after partial agent mutations.

## Decision

After failures, inspect repository state, classify the failure, and repair forward. Do not use
destructive reset/clean operations as routine recovery.

## Consequences

Failure history remains auditable and recovery is safer in multi-agent environments.
