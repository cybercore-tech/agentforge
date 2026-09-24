# Plan: P2-M025 — Canonical repository identity on cybercore-tech

Status: Approved
Milestone: P2-M025
Created: 2026-09-23
Owner: AgentForge project

## Goal

Make every repository-owned link, metadata field, and public page point at the new canonical home,
`https://github.com/cybercore-tech/agentforge`, with the project site at
`https://cybercore-tech.github.io/agentforge/`. Bring the README current with the work completed
since P2-M019 (gates, CI observation, batch launch, daemon fixes, remote-worker foundations).

## Non-goals

- No crate rename, package identity change, or publication (P2-M019 to P2-M022 policy stands).
- No history rewrite; plan and closure records that name `darkstardevx` stay as historical
  evidence.
- No change to the `darkstardevx/agentforge` repository itself. It keeps the history up to
  `976c4f9` and is not configured as a mirror here.
- No workflow logic changes.

## Context

On 2026-09-23 the full history (`976c4f9`, 13 branches) was pushed to the empty
`cybercore-tech/agentforge`. Pages was enabled with workflow builds and deployed successfully, the
repository description and homepage were set, and the local `origin` now points there. P2-M024 was
the first milestone pushed to the new repository. Repository-owned references still point at
`darkstardevx`: `Cargo.toml` `repository`/`homepage`, README badges, links, and clone command, the
Pages site (`site/index.html`, `site/script.js`), `docs/REGISTRY.md`, and the `CHANGELOG.md`
compare links. The README status line still says "P2-M019 registry identity policy complete", and
"What works today" and "Roadmap" predate P1-M004 to P2-M024.

## Architecture placement

Metadata and documentation only: workspace `Cargo.toml`, `site/`, README, CHANGELOG, and docs.

## Data flow

Not applicable; no runtime behavior changes.

## Invariants

- No crate or binary name changes; `cargo package` metadata stays valid.
- The site stays self-contained static HTML/CSS/JS served by the existing Pages workflow.
- Historical records (plans, PROJECT_STATE/AGENT_HANDOFF evidence) are not rewritten.

## ADRs

- ADR-0041: `cybercore-tech/agentforge` is the canonical repository; `darkstardevx/agentforge` is
  the historical location.

## Public API / CLI

None.

## Compatibility analysis

GitHub keeps the old repository available, so existing links keep working. Crate metadata only
changes URLs.

## Dependency analysis

None.

## Expected file boundary

- `.plans/P2-M025-canonical-repository-identity.plan.md`
- `.plans/ACTIVE`
- `Cargo.toml`
- `.cargo/registry-preflight.toml` (amendment 1)
- `site/index.html`
- `site/script.js`
- `README.md`
- `CHANGELOG.md`
- `docs/REGISTRY.md`
- `docs/RELEASE.md`
- `docs/adr/ADR-0041-canonical-repository.md`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

## Amendment 1 — package preflight regression

Running the planned `./scripts/package-preflight` gate failed with "no matching package named
`agentforge-ci` found". The same failure reproduces on `HEAD` without P2-M025 changes.
Classification: dependency/toolchain. P1-M004 and P1-M005 added `agentforge-gate` and
`agentforge-ci` as registry-versioned dependencies of `agentforge-platform`, but the preflight's
`[patch.crates-io]` table in `.cargo/registry-preflight.toml` only lists the internal crates that
existed when P2-M022 wrote it. The preflight is opt-in and not part of CI, so nothing caught it.
Repair: add both crates to the patch table.

## Test-first matrix

- `git grep darkstardevx` outside historical plan/state records returns only intentional
  historical-location notes.
- `cargo metadata` reports the new `repository` and `homepage`.
- `./scripts/package-preflight` still produces and inspects the archive.
- The deployed site links resolve to `cybercore-tech` (checked after the Pages deployment).

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Update metadata, site, README, CHANGELOG, and docs; add ADR-0041.
4. Run the full gate and the package preflight.
5. Push, deploy Pages, verify the live site, record CI evidence, and close.

## Failure modes

- If Pages fails to deploy from the new repository, the previous deployment stays live; classify
  as infrastructure and repair forward.

## Documentation impact

This milestone is documentation: README status, badges, capabilities, roadmap, and docs map;
CHANGELOG unreleased entries; REGISTRY and RELEASE URLs; ADR-0041.

## Quality gates

- `./scripts/gate.sh full`;
- `./scripts/package-preflight`;
- exact-SHA CI green on all seven jobs and a successful Pages deployment from the new repository.

## Acceptance criteria

- [ ] All repository-owned links and metadata name `cybercore-tech/agentforge`.
- [ ] README reflects the current capabilities and roadmap.
- [ ] Pages deploys from the new repository with correct links.
- [ ] Full local validation and exact-SHA CI evidence are recorded before closure.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
