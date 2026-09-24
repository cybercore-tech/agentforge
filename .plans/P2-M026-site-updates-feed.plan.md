# Plan: P2-M026 — Project site updates feed

Status: Approved
Milestone: P2-M026
Created: 2026-09-23
Owner: AgentForge project

## Goal

Add an "Updates" section to the project site that shows recently shipped milestones, work in
progress, per-phase progress, and the new features waiting for the next release. The section is
generated from the records every milestone closure already maintains, so it cannot drift from the
repository and needs no manual site edits.

## Non-goals

- No hand-maintained news content, blog, or feed subscriptions (RSS/Atom).
- No third-party scripts, analytics, fonts, or build toolchains; the site stays static HTML, CSS,
  and vanilla JS.
- No change to the milestone, plan, or changelog formats.
- No change to CI job structure beyond one validation step.

## Context

The site (P2-M017, retargeted in P2-M025) describes the workflow but not progress. P1-M004 through
P2-M025 shipped on 2026-09-23 with no visible trace on the site. The source data already exists:
`docs/MILESTONES.md` (ID, status, title, acceptance signal per phase), each plan's `Completed:`
line, and the CHANGELOG's Unreleased Added/Changed/Fixed sections. Some older plans record their
completion date in other formats ("Completed: pending" or no date), so a fallback is needed.

## Architecture placement

- `scripts/site-updates` (Python 3, standard library only, like `scripts/check-text-files`)
  parses the three sources into `updates.json`:
  - phases (sorted by phase number) with each milestone's ID, status, title, and signal;
  - a per-phase completed/total count;
  - completion dates, taken from the plan's `Completed: YYYY-MM-DD` or, failing that, the date of
    the last commit touching the plan file;
  - the most recently completed milestones (newest first) and all `active` ones;
  - the CHANGELOG Unreleased entries;
  - the source commit (`GITHUB_SHA` when present).
  It fails with an error rather than emitting an empty feed if no milestones or phases parse.
- `.github/workflows/pages.yml` checks out full history, runs the generator into `site/`, and
  validates the new section markers before upload.
- `.github/workflows/ci.yml`: the repository-policy job runs the generator to a temporary path,
  so a malformed milestone table or changelog fails CI instead of breaking a later deployment.
- `site/`: a new `#updates` section and nav link. `script.js` fetches `updates.json` and renders
  with DOM APIs and `textContent` only (no `innerHTML` for data). Backtick spans become `<code>`
  elements. Without the feed (local `file://` preview or a failed fetch), the section shows links
  to `docs/MILESTONES.md` and `CHANGELOG.md`.

## Data flow

MILESTONES.md + plans + CHANGELOG → `scripts/site-updates` (Pages build) → `site/updates.json` →
browser fetch → rendered timeline, progress bars, and changelog lists.

## Invariants

- The site never shows milestone data that does not come from committed repository records.
- Data is rendered as text, never as HTML.
- The generator is deterministic for a given checkout, apart from the source commit field.
- The existing Pages validation checks and README dialog keep working.

## ADRs

- ADR-0042: the site's updates feed is generated from milestone records at deploy time.

## Public API / CLI

- `scripts/site-updates [--output <path>]`, which defaults to `site/updates.json`.

## Compatibility analysis

Additive. The generated JSON is not committed (it is a build artifact) and is ignored locally.

## Dependency analysis

No new dependency. Python 3 is already required by `scripts/check-text-files`.

## Expected file boundary

- `.plans/P2-M026-site-updates-feed.plan.md`
- `.plans/ACTIVE`
- `.gitignore`
- `scripts/site-updates`
- `site/index.html`
- `site/styles.css`
- `site/script.js`
- `.github/workflows/pages.yml`
- `.github/workflows/ci.yml`
- `docs/SITE.md`
- `docs/adr/ADR-0042-site-updates-feed.md`
- `README.md`
- `CHANGELOG.md`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

## Test-first matrix

- The generator parses every phase and milestone currently in `docs/MILESTONES.md`, and the counts
  match the table rows.
- Completion dates resolve for plans with `Completed: YYYY-MM-DD` and fall back to git dates
  otherwise.
- The CHANGELOG Unreleased entries parse, including wrapped bullet lines.
- A missing or empty milestone table makes the generator exit non-zero.
- The page renders the section from a locally served build, and falls back to links when
  `updates.json` is missing.
- The live Pages deployment serves `updates.json`, and the section renders.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Write the generator and verify it against the repository.
4. Add the site section, styles, and renderer; verify with a local static server.
5. Wire the Pages build and the CI validation step.
6. Document it (`docs/SITE.md`, README, CHANGELOG, ADR-0042).
7. Run the full gate, push, dispatch CI and Pages, verify the live site, and close.

## Failure modes

- A generator error fails the Pages build before upload, so the previous deployment stays live.
- A fetch failure on the client shows the fallback links.

## Documentation impact

`docs/SITE.md` explains the site, the feed's sources, and how closures appear on it. README
mentions the updates section. The CHANGELOG records the feature. ADR-0042 records the decision.

## Quality gates

- `./scripts/gate.sh full`;
- the generator run against the repository;
- dispatched CI green on all seven jobs, a successful Pages deployment, and live verification.

## Acceptance criteria

- [ ] The site shows recently shipped milestones, in-progress work, phase progress, and
      unreleased features.
- [ ] The feed is generated from repository records at deploy time and validated in CI.
- [ ] Data is rendered safely as text, with a working fallback.
- [ ] Full local validation, CI, and live-site evidence are recorded before closure.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
