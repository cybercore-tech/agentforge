# Plan: P0-M015 — Wall-clock audit timestamps

Status: Complete
Milestone: P0-M015
Created: 2026-09-24
Owner: AgentForge project

## Goal

Every audit event records when it happened (dogfooding finding 12). Events are stamped with
wall-clock milliseconds when they are created. The audit store stamps any event that still carries
a placeholder, so no future event can be stored without a time. The HUD shows each event's time and
each agent run's duration, so a run like P0-M014 can be timed from evidence alone.

## Non-goals

- No change to the audit format: the `timestamp` field already exists and is hashed. It has always
  been filled with the placeholder `1`.
- No rewriting of existing records: legacy events keep `1` and display as having no time.
- No time-zone handling: times are shown in UTC (ISO 8601, `Z`).

## Context

While reviewing the P0-M014 remote run, the run's timing (claim to import: 7 min 14 s) had to be
measured outside AgentForge. Ten of the twelve production `AuditEvent::new` calls passed the
placeholder `1` (the other two were the CI observation events, which also pass `1`; all eleven
current production sites do). Nothing reads timestamps today. For remote work, timing is evidence:
lease timelines, slow gates, and incident review.

Agent-run events are built in an in-memory attempt log and persisted as one batch at the end of the
run. Stamping at persist time would give the start and finish of a run the same time, so events
must be stamped **at creation**. The store's stamp is only a backstop.

## Architecture placement

- **`agentforge-audit`:**
  - `wall_clock_ms()`, and `AuditEvent::now(sequence, id, kind, actor)`, which stamps the current
    time;
  - `UNSET_TIMESTAMP_BELOW` (2000-01-01 in ms): a timestamp below it means "unset";
  - `FileAuditStore::append` and `append_batch` stamp unset events with the append time before
    hashing, and leave set timestamps unchanged;
  - `AuditEvent::new` is kept for tests and explicit times.
- **Callers:** all eleven production sites (orchestrator 5, operator 5, leases 1) use
  `AuditEvent::now`.
- **`agentforge-hud`:**
  - `audit_recent` lines gain `at=YYYY-MM-DDTHH:MM:SSZ` for stamped events (none for legacy ones),
    using a std-only civil-date conversion;
  - `AgentRunSummary` gains `started_at_ms` and `finished_at_ms`, the run line gains
    `duration=<seconds>s` when both are stamped, and legacy runs show no duration.
- **Docs:** AUDIT.md (timestamp semantics), HUD.md (the new fields), and DOGFOODING (finding 12
  resolved).

## Invariants

- Every event appended from now on has a wall-clock timestamp.
- Timestamps never change the ordering or integrity rules: sequence and the digest chain stay
  authoritative.
- Legacy records verify and render unchanged, without a time.

## ADRs

None; this fills an existing field. AUDIT.md records the semantics.

## Public API / CLI

`wall_clock_ms`, `AuditEvent::now`, `UNSET_TIMESTAMP_BELOW`, and the HUD `at=` and `duration=`
fields.

## Compatibility analysis

The format is unchanged. Readers ignoring timestamps are unaffected. HUD output gains fields.

## Dependency analysis

None (std only).

## Expected file boundary

- `.plans/P0-M015-wall-clock-audit-timestamps.plan.md`, `.plans/ACTIVE`
- `crates/agentforge-audit/src/lib.rs`, `crates/agentforge-audit/tests/*.rs`
- `crates/agentforge-orchestrator/src/lib.rs`, `crates/agentforge-orchestrator/tests/*.rs`
- `crates/agentforge-operator/src/lib.rs`, `crates/agentforge-operator/src/leases.rs`
- `crates/agentforge-hud/src/lib.rs`
- `crates/agentforge-cli/tests/*.rs`
- `docs/AUDIT.md`, `docs/HUD.md`, `docs/DOGFOODING.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

- **Audit:** `now()` falls within the call window; the store stamps an unset event with a time
  inside the append window and leaves a set time alone; it round-trips; a batch keeps distinct
  per-event times.
- **Orchestrator:** after a launch, `AgentStarted` and `AgentFinished` both carry real times, and
  the finish is not before the start.
- **HUD:** the date conversion on fixed epochs (the Unix epoch, a leap day, and a known 2026 time);
  `at=` shown for stamped events and absent for legacy ones; `duration=` computed from the run's
  start and finish.
- **CLI:** after a real launch, `forge hud` shows `at=` and `duration=`.
- **Guard:** no production call site still passes `1` (a source check in the orchestrator or HUD
  test).

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Audit crate, then the callers, then the HUD, with tests at each step.
4. Docs; gate; commit (exit checked); push; CI plus a repeat.
5. Close; tag after the dry run names the "close ..." commit.

## Failure modes

- A clock before 2000 would make stamps read as unset. That is not a real deployment; the store
  would stamp at append.
- Clock jumps give odd durations. The durations are informational, and ordering stays sequence
  based.

## Documentation impact

AUDIT.md, HUD.md, and DOGFOODING. README, CHANGELOG, and MILESTONES at closure.

## Quality gates

- `./scripts/gate.sh full`;
- push CI plus a dispatched repeat.

## Acceptance criteria

- [x] Every new audit event has a wall-clock timestamp, stamped at creation (with a store backstop).
- [x] The HUD shows event times and agent-run durations; legacy records render without them.
- [x] Docs; CI evidence; closed and tagged correctly.

## Completion record

Implementation commit: `bb1aff6`
CI run: `36105499481` (push) and `36105880664` (dispatched repeat)
CI result: green on all seven jobs in both runs
Completed: 2026-09-25
Notes:
- All 11 production call sites now stamp events at creation. The guard test was proven with a
  probe file (it failed and named the file) before the probe was removed.
- The HUD's date conversion was cross-checked against Python's `datetime` on four fixed epochs. My
  first hand-computed 2026 expectation in the test was wrong and the implementation was right; the
  expectation was corrected from the reference, not guessed.
- Legacy records (timestamp `1`) render unchanged, without a time. From now on, remote and local
  runs can be timed from the audit log alone.
- This completes the operator's ordered fixes: finding 14 and 13 (P4-M009), then 12 (this
  milestone).
