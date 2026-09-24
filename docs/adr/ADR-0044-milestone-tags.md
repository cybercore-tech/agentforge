# ADR-0044: Milestone tags mark closure commits; release tags stay separate

- Status: Accepted
- Date: 2026-09-24
- Milestone: P2-M030

## Context

Until 2026-09-24 the repository had no tags. Milestone evidence lived in plans, state documents,
and commit messages, but there was no stable Git ref for "the commit that closed a milestone". The
operator asked that important points be tagged. The release workflow already reacts to `v*.*.*`
tags, so any new tag scheme must not start a release by accident.

## Decision

- Each completed milestone gets an annotated tag `milestone/<ID>` on its closure commit. That is the
  commit that introduced `Status: Complete` in its plan, or, for older plans, the plan's last
  commit. When a milestone has several plans (for example a repair plan), the latest closure wins.
- The tag message carries the milestone title and acceptance signal from `docs/MILESTONES.md` and
  the plan's completion record.
- `scripts/tag-milestone` creates the tags. It only tags completed milestones on commits in `HEAD`'s
  history, is idempotent, and never moves or deletes a tag.
- Tagging is the final closure step (AGENTS.md rule 14). Release tags (`vX.Y.Z`) remain a separate,
  explicit decision.

## Consequences

Positive:

- milestones are addressable with ordinary Git: `git show`, checkout, and ranges between milestones;
- the tag message keeps the evidence next to the commit it describes;
- the `milestone/` namespace cannot trigger releases.

Trade-offs:

- tags are unsigned until the P5 signing work;
- a closure that is later amended keeps its original tag; follow-up work gets its own milestone.
