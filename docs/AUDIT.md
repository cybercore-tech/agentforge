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

## Timestamps (P0-M015)

Every event records **when it happened**, in wall-clock milliseconds since the Unix epoch.

- **Stamped at creation.** Production code builds events with `AuditEvent::now`. An agent run's
  events are persisted in one batch when the run finishes, and each keeps its own creation time.
  A guard test fails if production code uses the explicit-time constructor.
- **Store backstop.** Any event appended with a timestamp below `UNSET_TIMESTAMP_BELOW`
  (2000-01-01) is stamped with the append time before hashing. Real timestamps are never changed.
- **Integrity and order are unchanged.** The sequence and the digest chain stay authoritative;
  timestamps are evidence, not ordering. Clock jumps can give odd durations but never reorder
  records.
- **Legacy records** (written before P0-M015) carry the placeholder `1`. They verify and render as
  before, with no time.

The HUD shows `at=YYYY-MM-DDTHH:MM:SSZ` (UTC) on recent events and `duration=<seconds>s` on agent
runs (see HUD.md).
