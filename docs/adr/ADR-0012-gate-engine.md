# ADR-0012: Explicit gate definitions and structured process evidence

- Status: Proposed
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

AgentForge needs reusable local validation evidence without shell snippets or treating a process
exit as task acceptance.

## Proposed decision

P0-M007 will define named explicit process gates. A standard-library runner will execute direct
argument vectors in a caller-supplied directory with explicit environment, deadline, shared output
limit, and structured report.

## Consequences

Later policy, audit, classifier, and vertical-slice milestones can consume a stable gate report.
Gate execution remains independent of task lifecycle and repair decisions.
