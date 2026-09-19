# ADR-0003: Durable state lives outside model memory

- Status: Accepted
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

Agent context windows and conversations are not reliable long-term project databases.

## Decision

Project state, task state, plans, permissions, decisions, and evidence are persisted in repository
files or AgentForge-owned local storage.

## Consequences

Any compatible agent can resume work from durable project context. Chat history is helpful context
but never the sole source of truth.
