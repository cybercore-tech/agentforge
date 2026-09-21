# Plan: P2-M017 — AgentForge GitHub Pages

Status: Approved
Milestone: P2-M017
Owner: AgentForge project

## Objective

Ship a polished, static GitHub Pages landing page for AgentForge that introduces the product as a
local-first, evidence-preserving environment for AI-assisted software work. The page will use the
approved “forge rail” visual language from the P2-M017 mockup while remaining distinct from the
project’s existing documentation and README surfaces.

## Scope

- Build a dependency-free, semantic static page under `site/`.
- Establish the approved visual system: ink/navy canvas, warm amber intent, cyan execution, green
  verification, technical grid, and the PLAN → ISOLATE → RUN → REVIEW → INTEGRATE rail.
- Explain the product through concise sections for the workflow, safeguards, and operator entry
  points, with clear alpha/pre-release messaging.
- Provide responsive layout, keyboard-visible focus states, reduced-motion behavior, and readable
  contrast without relying on JavaScript or remote fonts.
- Add a least-privilege GitHub Pages deployment workflow that publishes `site/` on pushes to `main`
  and supports manual dispatch.
- Link the page from the README and preserve the approved visual mockup as a design reference.
- Record the static-site and deployment decision in an ADR.

## Non-goals

- No changes to the Rust crates, CLI behavior, daemon behavior, or project-local `.forge/` state.
- No analytics, authentication, remote API calls, contact collection, or client-side application
  runtime.
- No release-readiness claim; the page must continue to identify AgentForge as alpha and not
  release-ready.
- No replacement of the README or documentation corpus.

## File boundary

Implementation is limited to:

- `.plans/ACTIVE`
- `.plans/P2-M017-agentforge-github-pages.plan.md`
- `.github/workflows/pages.yml`
- `site/index.html`
- `site/styles.css`
- `docs/assets/agentforge-pages-mockup-v1.png`
- `docs/adr/ADR-0029-agentforge-github-pages.md`
- `README.md`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

## Validation

- Repository text and plan policy validation passes.
- The local full gate passes without changing Rust behavior.
- Static-site checks confirm the expected page entry point, semantic landmarks, required navigation
  anchors, alpha warning, and deployment workflow references.
- GitHub Actions deployment uses the exact pushed commit and publishes the `site/` artifact with
  `pages: write` only in the deploy job.
- The final implementation and closure commits receive green exact-SHA CI evidence.

## Acceptance criteria

- [ ] A visitor can understand AgentForge’s purpose, workflow, and safety posture from the landing
      page without reading the source code.
- [ ] The page visibly implements the approved forge-rail concept and responsive card layout.
- [ ] Primary calls to action lead to the workflow documentation and repository/CLI entry points.
- [ ] The page remains usable on narrow screens, keyboard navigation, and reduced-motion settings.
- [ ] GitHub Pages deployment is reproducible from `main` or manual dispatch with bounded workflow
      permissions and no third-party build dependency.
- [ ] README discovery, ADR rationale, mockup reference, and milestone state are updated.
- [ ] Local gate and exact implementation/closure CI evidence are recorded before completion.

## Plan notes

The page is a presentation layer only. AgentForge’s repository, task contracts, approvals, audit
records, and exact-head evidence remain the authoritative product surfaces.
