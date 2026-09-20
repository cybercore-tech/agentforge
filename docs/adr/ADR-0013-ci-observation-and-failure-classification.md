# ADR-0013: Read-only CI observation and conservative failure classification

- Status: Accepted
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

Remote CI evidence is only valid for the exact commit SHA it tested. AgentForge also requires a
failure classification before repair, without granting a parser or provider adapter authority to
change CI, source, or task state.

## Proposed decision

P0-M008 will use a configured direct command as a read-only CI provider boundary. It will accept a
strict versioned observation protocol, bind selected run evidence to a requested full SHA, and
classify supplied failed-job evidence conservatively into the repository failure taxonomy.

## Consequences

Provider authentication and network behavior remain operator concerns outside the core crate.
Malformed, ambiguous, stale, or insufficient evidence produces an explicit error or unknown
classification rather than automatic retry or repair. Later audit and policy components can store
and act on the returned immutable evidence.
