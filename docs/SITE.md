# Project site

The public site at <https://cybercore-tech.github.io/agentforge/> is static HTML, CSS, and vanilla
JavaScript in `site/`, deployed by `.github/workflows/pages.yml`. It uses no third-party scripts,
fonts, or build toolchain.

## Updates section

The **What's new** section (`#updates`) shows:

- **Recently shipped**: the eight most recently completed milestones, newest first, each linking
  to its plan and evidence;
- **In progress**: every milestone whose status is `active`;
- **Phase progress**: completed/total milestones per phase;
- **Coming in the next release**: the CHANGELOG's `[Unreleased]` Added, Changed, and Fixed entries.

Nothing in it is written by hand. `scripts/site-updates` (Python 3, standard library only) builds
`site/updates.json` from records every milestone already maintains:

| Source | Used for |
| --- | --- |
| `docs/MILESTONES.md` phase tables | IDs, status, titles, acceptance signals, phase totals |
| `.plans/<ID>-*.plan.md` `Completed: YYYY-MM-DD` | Completion dates |
| Last commit touching the plan | Fallback date for older plans, and ordering within a day |
| `CHANGELOG.md` `[Unreleased]` | Upcoming release notes |

The feed is a build artifact: the Pages workflow generates it (with full Git history) before
upload, and `site/updates.json` is gitignored. The CI repository-policy job also runs the
generator, so a malformed milestone table or changelog fails CI instead of a later deployment.
The generator exits non-zero rather than publishing an empty feed.

`site/script.js` renders the feed with DOM text APIs only. Backtick spans become `<code>`
elements and no record text is parsed as HTML. If the feed cannot be loaded (for example when
opening `site/index.html` from disk), the section says so and keeps its links to the full
milestone list, changelog, and decision records.

## How a milestone reaches the site

Nothing extra is required beyond the normal milestone workflow:

1. The implementation commit adds the milestone row (`active`), which shows under *In progress*.
2. The implementation or closure commit adds CHANGELOG `[Unreleased]` entries.
3. The closure commit marks the row `complete` and records `Completed: YYYY-MM-DD` in the plan.
4. The next Pages deployment publishes it. Pushes to `main` trigger it; while push events do not
   fire on the canonical repository, dispatch `AgentForge Pages` manually.

## Local preview

```bash
./scripts/site-updates            # writes site/updates.json
cd site && python3 -m http.server 18765 --bind 127.0.0.1
```

Open <http://127.0.0.1:18765/>. The page fetches `updates.json`, so a plain `file://` open shows
the fallback instead of the feed.
