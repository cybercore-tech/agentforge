# Plan: P2-M030 — Milestone tags

Status: Draft
Milestone: P2-M030
Created: 2026-09-24
Owner: AgentForge project

## Goal

Mark every completed milestone in Git history with an annotated tag. The tag's message carries the
milestone's title, acceptance signal, and completion record, so the most important points in the
project's history can be found, checked out, and compared with ordinary Git tooling. Tagging becomes
the last step of every milestone closure.

## Non-goals

- No release tags. `vX.Y.Z` tags trigger the release workflow and remain a separate, explicit
  release decision (`docs/RELEASE.md`).
- No signed tags. Signing belongs to the planned P5 signing and provenance work.
- No rewriting of history or of existing plan records.

## Context

The repository had no tags at all on 2026-09-24. Milestone evidence lives in plans,
`PROJECT_STATE.md`, and commit messages, but there is no stable ref for "the commit that closed
P2-M023". The operator asked that important things be tagged. The release workflow matches only
`v*.*.*`, so a separate `milestone/` namespace cannot trigger a release.

## Architecture placement

`scripts/tag-milestone` (Python 3, standard library):

- `scripts/tag-milestone <ID>` creates `milestone/<ID>` on that milestone's **closure commit**: the
  commit that introduced `Status: Complete` in `.plans/<ID>-*.plan.md` (found with
  `git log -S`), or, for old plans that never recorded it, the last commit that touched the plan.
- The tag message includes the ID, title and acceptance signal from `docs/MILESTONES.md`, and the
  plan's `Implementation commit`, `CI run`, `CI result`, and `Completed` lines.
- Only milestones with status `complete` in `docs/MILESTONES.md` are tagged.
- It is idempotent: an existing tag on the same commit is left alone; an existing tag on a
  different commit is an error and is never moved.
- `--all` backfills every completed milestone. `--dry-run` prints the plan without tagging.
  `--self-test` checks the parsing on built-in samples (run in CI).
- Pushing stays explicit (`git push <remote> 'refs/tags/milestone/*'`) and is documented.

## Invariants

- Tags are annotated and never moved or deleted by the script.
- Only completed milestones are tagged, and only on commits in `main`'s history.
- The `milestone/` namespace never matches the release trigger `v*.*.*`.

## ADRs

- ADR-0044: milestone tags mark closure commits; release tags stay separate.

## Public API / CLI

- `scripts/tag-milestone [<ID> | --all] [--dry-run]` and `--self-test`.

## Compatibility analysis

Additive. Existing workflows ignore `milestone/*` tags.

## Dependency analysis

None.

## Expected file boundary

- `.plans/P2-M030-milestone-tags.plan.md`
- `.plans/ACTIVE`
- `scripts/tag-milestone`
- `.github/workflows/ci.yml` (self-test step)
- `AGENTS.md`
- `docs/OPERATIONS.md`
- `docs/GOVERNANCE.md`
- `docs/RELEASE.md`
- `docs/adr/ADR-0044-milestone-tags.md`
- `README.md`
- `CHANGELOG.md`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

## Test-first matrix

- The self-test resolves the closure commit and message from sample plan and milestone text, and
  rejects unknown or incomplete milestones.
- `--all --dry-run` lists every completed milestone with a resolvable closure commit, and flags
  any that cannot be resolved.
- Running the tagger twice is a no-op the second time; a conflicting existing tag is an error.
- After the backfill and push, `git ls-remote --tags origin 'milestone/*'` matches the local tags,
  and no release workflow run is triggered.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Write the script with its self-test, dry-run it on the repository, and review the mapping.
4. Update the docs, add the CI step, and commit and push the implementation.
5. Backfill tags for all completed milestones and push them.
6. Close, then tag P2-M030 itself as the first tag created by the new closure step.

## Failure modes

- An unresolvable closure commit is reported and skipped, never guessed.
- A conflicting existing tag stops the run for that milestone.

## Documentation impact

AGENTS.md (closure step), OPERATIONS.md (tagging commands), GOVERNANCE.md, RELEASE.md (milestone
versus release tags), README, CHANGELOG, ADR-0044.

## Quality gates

- `./scripts/gate.sh full`;
- the script self-test in CI;
- push-triggered CI green, with tags visible on the remote.

## Acceptance criteria

- [ ] Every completed milestone has an annotated `milestone/<ID>` tag on its closure commit.
- [ ] Tagging is part of the documented closure workflow.
- [ ] Release tags remain separate and untriggered.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
