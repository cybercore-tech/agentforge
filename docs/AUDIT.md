# Audit log

P0-M009 provides `agentforge-audit`, a standard-library-only append-only local evidence store.
Callers supply explicit sequence numbers, event IDs, event kinds, actors, injected timestamps,
optional task IDs, and bounded key/value fields. The log records facts; it never grants approval,
accepts a task, or initiates repair.

Files begin with the `AFAL` versioned header. Each record is a bounded little-endian length frame
containing canonical UTF-8 event fields, the previous record digest, and a deterministic chained
digest. `FileAuditStore::open` verifies the complete file before returning. Partial frames, gaps,
duplicates, unknown versions or event kinds, field-limit violations, reordered or modified records,
and trailing bytes fail closed without truncating or repairing the source.

Successful appends use an append-only file handle and `sync_all` before returning. The digest is a
tamper-evident integrity signal, not cryptographic authenticity without external key custody.
Queries return sequence order and are bounded by the decoded record limits.
