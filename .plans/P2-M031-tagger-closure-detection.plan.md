# Plan: P2-M031 — Tagger closure detection fix

Status: Complete
Milestone: P2-M031
Created: 2026-09-24
Owner: AgentForge project

## Goal

Make `scripts/tag-milestone` find a milestone's closure commit from the plan's actual status line,
and verify it before tagging, so a plan that merely mentions the words `Status: Complete` can never
be tagged at the wrong commit.

## Non-goals

- No change to the tag namespace, message format, or closure workflow.
- No change to any of the 55 backfilled tags, which were verified to point at "close ..." commits.

## Context

Right after P2-M030 closed, `scripts/tag-milestone P2-M030` tagged the **draft** commit `3d15a6a`,
not the closure `fa0672c`. The P2-M030 plan's own text describes "the commit that introduced
`Status: Complete`", and `git log -S` matches any text, so the draft already "introduced" the
string. The tag had been pushed. The operator's tooling error was corrected immediately: the tag was
deleted locally and on the remote about a minute after creation, before anything referenced it.
That is recorded here as the only exception to the never-move and never-delete rule. The 55 backfill
tags were reviewed in the dry run and all point at closure commits.

Classification: semantic/test (substring match instead of line match).

## Architecture placement

`scripts/tag-milestone` only:

- Find candidate commits with `git log -G '^Status: Complete[[:space:]]*$'`, which matches only an
  added or removed status line. Take the earliest commit whose version of the plan has that line.
- Verify each candidate by reading the plan at that commit (`git show <commit>:<plan>`) and
  requiring a `^Status: Complete\s*$` line; otherwise report an error and never tag.
- Self-test the status-line matcher against the P2-M030 failure shape (the phrase inside prose or
  backticks must not match).

## Invariants

- A tag is created only on a commit where the plan's status line reads `Status: Complete`.
- Existing correct tags are untouched.

## ADRs

ADR-0044 gains an addendum recording the exception and the verification rule.

## Public API / CLI

None.

## Compatibility analysis

`--all --dry-run` must still resolve all 55 previously tagged milestones to the same commits.

## Dependency analysis

None.

## Expected file boundary

- `.plans/P2-M031-tagger-closure-detection.plan.md`
- `.plans/ACTIVE`
- `scripts/tag-milestone`
- `docs/adr/ADR-0044-milestone-tags.md`
- `docs/OPERATIONS.md`
- `CHANGELOG.md`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

## Test-first matrix

- The self-test's status-line matcher accepts `Status: Complete` and rejects the prose and backtick
  forms from the P2-M030 plan.
- `--all --dry-run` reports "already tags" for all 55 existing tags, with no errors.
- `scripts/tag-milestone P2-M030` resolves to `fa0672c`.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Fix and verify the script.
4. Push and verify CI; tag P2-M030 correctly; close P2-M031 and tag it.

## Failure modes

- If verification fails for any milestone, that milestone is reported and not tagged.

## Documentation impact

ADR-0044 addendum; OPERATIONS.md note; CHANGELOG Fixed entry.

## Quality gates

- `./scripts/gate.sh full`;
- the tagger self-test in CI;
- push-triggered CI green.

## Acceptance criteria

- [x] Closure detection uses the status line and is verified before tagging.
- [x] All 55 existing tags are confirmed unchanged by the dry run.
- [x] P2-M030 and P2-M031 are tagged on their closure commits.

## Completion record

Implementation commit: `12796ef`
CI run: `36005319580` (push), `36005682117` and `36005691600` (dispatched) are green; `36005350819` (dispatched) failed
CI result: three of four runs green on all seven jobs. The failing run is an unrelated pre-existing macOS daemon flake (below).
Completed: 2026-09-24
Notes:
The dry run confirmed all 55 earlier tags unchanged. `milestone/P2-M030` was recreated on its closure
`fa0672c` and pushed. Dispatched run `36005350819` failed only on macOS 14:
`long_daemon_executions_succeed_while_status_stays_available` (P2-M024), whose final `forge daemon
stop` returned `daemon I/O failed: Invalid argument (os error 22)`. The same commit passed that job in
the other three runs. Classification: a pre-existing intermittent macOS daemon-client transport flake,
not related to the tagger change. It is scheduled as its own milestone (P2-M032) rather than hidden.
