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

## Addendum (P2-M031, 2026-09-24)

The first run of the closure step tagged P2-M030's **draft** commit (`3d15a6a`), not its closure
(`fa0672c`). Closure detection used `git log -S "Status: Complete"`, and the P2-M030 plan's own prose
mentions those words. The tag had been pushed. It was deleted locally and on the remote about a
minute after creation, before anything referenced it, and recreated on the closure commit. **This is
the only exception to the rule that milestone tags are never moved or deleted.** It was allowed
because the tool itself had just created the wrong tag.

Since P2-M031 the closure commit is the earliest commit whose plan has a status line beginning
`Status: Complete` at the start of a line, verified by reading the plan at that commit before
tagging. The dry run confirmed all 55 earlier tags were already on their closure commits.

## Addendum (P2-M034, 2026-09-24)

The P0-M014 closure exposed another gap (dogfooding finding 15). The closure commit was rejected
unnoticed, and the tagger still tagged and pushed the plan's **approve** commit. It read the
milestone table from the working tree, where the uncommitted closure already said `complete`, and
then applied the legacy last-commit fallback. The tag was deleted within about two minutes and
recreated on the real closure. The tagger now works as follows:

- **Committed state only.** `docs/MILESTONES.md`, the plan list, and completion records are read
  from `HEAD`. Uncommitted changes to the milestone table or the milestone's plans are refused:
  "commit the closure first".
- **The legacy fallback is legacy only.** The last commit touching a plan is used only when no
  committed version of that plan ever had a `Status:` line. A plan with status lines but no
  committed `Status: Complete` contributes no closure. If no plan of the milestone is closed, the
  milestone is refused.
- **Existing tags are unchanged.** A tag created before P2-M034 that matches the old rule stays
  valid and is reported as `(pre-P2-M034 closure rule)`. Only `milestone/P2-M017` is in that state:
  its repair plan was closed without updating its status line. A tag that neither rule would choose
  is still an error, and tags are never moved.
- `--self-test`, run by CI, replays these cases in throwaway repositories: the P0-M014 incident, a
  committed table with an unclosed plan, a proper closure, a legacy plan, prose mentioning the
  status line, and the repair-plan case.

The operator's dry-run subject check (AGENTS.md rule 14) remains required. The tagger is now a
second line of defence instead of the only one.
