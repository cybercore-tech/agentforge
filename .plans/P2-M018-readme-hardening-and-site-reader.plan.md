# Plan: P2-M018 — README hardening and in-page reader

Status: Approved
Milestone: P2-M018
Owner: AgentForge project

## Objective

Make the public AgentForge entry points honest, explicit, and easier to use by updating the
README to reflect the current P2-M017 state, expanding pre-release and operator-safety warnings,
and adding an accessible styled README reader to the GitHub Pages landing page. Add the repository
About homepage link so visitors have a consistent path from GitHub to the public project surface.

## Scope

- Update `README.md` status, capability descriptions, limitations, operator responsibilities,
  command guidance, and safe-first-run detail without claiming release readiness.
- Add a native, keyboard-accessible `<dialog>` to `site/index.html` for the existing “Read the
  README” call to action.
- Add a small local `site/script.js` controller for opening, closing, focus restoration, and
  unsupported-browser fallback. Do not fetch or embed the README at runtime.
- Style the dialog, backdrop, content, code samples, and narrow-screen layout in
  `site/styles.css`, including reduced-motion compatibility.
- Extend `.github/workflows/pages.yml` static checks for the dialog and local script boundary.
- Record the bounded local-reader decision in ADR-0030 and keep ADR-0029’s site constraints
  accurate.
- Record the milestone plan, completion evidence, and operator-facing state in the project
  milestone/state/handoff documents.
- Set the GitHub repository About homepage to `https://darkstardevx.github.io/agentforge/` and
  verify the metadata after the implementation is pushed.

## Non-goals

- No Rust crate, CLI, daemon, task-state, audit, or policy behavior changes.
- No remote README fetch, iframe, analytics, third-party JavaScript, authentication, or runtime
  API dependency.
- No attempt to make the README a stable API contract; the project remains alpha, incomplete, and
  not release-ready.
- No automatic repository description, topic, branch, or release changes beyond the requested
  About homepage link.

## File boundary

Implementation and closure documentation are limited to:

- `.plans/ACTIVE`
- `.plans/P2-M018-readme-hardening-and-site-reader.plan.md`
- `README.md`
- `site/index.html`
- `site/styles.css`
- `site/script.js`
- `.github/workflows/pages.yml`
- `docs/adr/ADR-0029-agentforge-github-pages.md`
- `docs/adr/ADR-0030-readme-reader-dialog.md`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

The GitHub repository About homepage is external metadata verified as milestone evidence, not a
source-file change.

## Validation

- The plan-only checkpoint passes repository policy and the full local gate before approval.
- README text checks confirm the current milestone, alpha/not-release-ready warning, explicit
  authority and safety limitations, and operator checklist are present.
- Static-site checks confirm a local script, native dialog, accessible labels, full README link,
  alpha warning, and reduced-motion styles.
- The full local gate passes without Rust behavior changes.
- The repository About homepage resolves to the public Pages URL.
- The implementation commit and all closure/evidence commits receive green exact-SHA CI; the Pages
  workflow deploys the same pushed SHA successfully.

## Acceptance criteria

- [ ] README status and capability detail match the completed P2-M017 baseline.
- [ ] README clearly states alpha status, missing guarantees, trust boundaries, and safe operator
      practices in explicit language.
- [ ] The landing-page “Read the README” control opens a styled, keyboard-accessible reader without
      leaving the page, with a clear link to the full README.
- [ ] Dialog behavior is local, bounded, responsive, reduced-motion aware, and has a safe fallback
      when native `<dialog>` is unavailable.
- [ ] GitHub Pages static validation covers the new reader assets and remains dependency-free.
- [ ] Repository About metadata, milestone records, and exact CI/Pages evidence are recorded.

## Plan notes

The reader is intentionally curated static content rather than a runtime copy of `README.md`.
This avoids network dependence and content injection while making the landing page useful in a
single interaction; the full README remains the canonical detailed document.

## Completion record

To be filled at closure with implementation SHA, exact CI run, Pages deployment run, closure SHA,
and final metadata verification.
