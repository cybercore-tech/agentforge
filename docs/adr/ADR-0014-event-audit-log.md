# ADR-0014: Append-only event audit log with integrity chaining

- Status: Proposed
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

AgentForge needs durable evidence of orchestration decisions and observations that survives model
replacement and can be checked after a crash or partial write. Existing task snapshots preserve
state but do not provide an append-only history.

## Proposed decision

P0-M009 will provide a standard-library-only append-only event log behind an audit-store boundary.
Records use canonical bounded framing, explicit contiguous sequences, and a chained digest. Replay
fails closed on corruption, gaps, unsupported versions, and incomplete records. Audit events remain
evidence and never grant authority or perform repair.

## Consequences

Local history becomes queryable and tamper-evident without committing to SQLite or a remote sink.
Digest chaining detects accidental or post hoc mutation but is not cryptographic authenticity
without external key custody. Future policy or remote backends can consume the event contract.
