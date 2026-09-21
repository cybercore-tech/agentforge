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

- [ ] The workflow contains the native GitHub expression and no literal backslash.
- [ ] Local gate passes.
- [ ] Exact CI and Pages deployment are green for the repaired SHA.
- [ ] Repair evidence is recorded and the active pointer is removed.
