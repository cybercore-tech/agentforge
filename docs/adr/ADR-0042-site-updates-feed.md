# ADR-0042: The site's updates feed is generated from milestone records

- Status: Accepted
- Date: 2026-09-23
- Milestone: P2-M026

## Context

The project site described the workflow but showed no progress; eight milestones shipped on
2026-09-23 without any visible trace. A hand-maintained news section would drift and would add a
manual step to every closure. The repository already records everything an updates section needs
in `docs/MILESTONES.md`, the plans' completion records, and the CHANGELOG.

## Decision

- `scripts/site-updates` generates `site/updates.json` from those records. It runs in the Pages
  build with full history, and completion dates come from the plans or their closure commits.
- The generated feed is not committed; CI runs the generator on every run to catch malformed
  sources early.
- The page renders the feed client-side with text-only DOM APIs and degrades to repository links
  when the feed is unavailable.

## Consequences

Positive:

- the site cannot claim progress the repository does not record;
- closing a milestone is the only step needed to publish it;
- no new dependency or build toolchain.

Trade-offs:

- the section needs JavaScript and a served (not `file://`) page;
- the feed reflects the last Pages deployment, which currently needs a manual dispatch;
- presentation quality depends on the wording of milestone titles, signals, and changelog entries.
