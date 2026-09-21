# Plan: P2-M017-R1 — Pages environment URL repair

Status: Approved
Milestone: P2-M017
Owner: AgentForge project

## Objective

Repair the GitHub Pages workflow environment URL expression so the deployment metadata exposes a
valid HTTPS URL without changing the published site or deployment authority.

## Scope

- Remove the accidental literal escape before the \${{ steps.deployment.outputs.page_url }}
  expression in \`.github/workflows/pages.yml\`.
- Re-run the local gate and exact CI/Pages workflows.
- Record the repair evidence and close this bounded follow-up.

## Non-goals

- No changes to the static page, Rust crates, deployment permissions, or repository Pages settings.
- No change to the public product messaging or alpha/pre-release status.

## File boundary

- \`.plans/ACTIVE\`
- \`.plans/P2-M017-pages-environment-url-repair.plan.md\`
- \`.github/workflows/pages.yml\`
- \`.plans/P2-M017-agentforge-github-pages.plan.md\`
- \`PROJECT_STATE.md\`
- \`AGENT_HANDOFF.md\`

## Acceptance criteria

- [x] The workflow contains the native GitHub expression and no literal backslash.
- [x] Local gate passes.
- [x] Exact CI and Pages deployment are green for the repaired SHA.
- [x] Repair evidence is recorded and the active pointer is removed.

## Completion record

Repair commit: `7d2b45b`.
Exact repair CI: `35554310599` — all seven jobs green across repository policy, stable, MSRV,
CLI smoke, Ubuntu, macOS, and Windows.
Exact repair Pages deployment: `35554310590` — artifact validation and deployment green with a
valid environment URL for the repaired SHA.
Completed: 2026-09-21
Notes: The repair removed only the accidental literal backslash from the Pages environment URL
expression; page content, permissions, and deployment source were unchanged.
