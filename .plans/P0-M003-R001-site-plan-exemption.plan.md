# Plan: P0-M003-R001 — Exempt the GitHub Pages site from implementation authority

Status: Draft
Milestone: P0-M003
Created: 2026-10-02

## Goal

Stop `validate-plan-policy` from classifying GitHub Pages site content under
`site/` as implementation, so site-only changes no longer fail the
Repository policy CI job.

## Non-goals

- No change to how Rust, workflow, script or manifest changes are classified.
- No change to plan statuses, the `.plans/ACTIVE` contract or ADR-0004.
- No hook bypass and no policy weakening for product code.

## Context

`main` is red. Commit b7085df (squash merge of PR #2, "add the shared
Cybercore Tech contact block") changed only `site/index.html`. The
Repository policy job failed with:

    HEAD commit contains implementation but 27577d1 has no .plans/ACTIVE

`is_implementation_path` treats every path that is not `.plans/ACTIVE` and
does not end in `.md` as implementation. `site/` is the static project
website deployed by `.github/workflows/pages.yml`. It is documentation for
humans, like the Markdown files that are already exempt, and it is not
compiled into or shipped with any crate.

## Architecture placement

Repository workflow infrastructure only (`tools/xtask`).

## Data flow

No runtime data flow changes.

## Invariants

- Paths under `site/` are control (non-implementation) changes.
- Every other non-Markdown path keeps its current classification.
- A path that only starts with the letters `site` (for example `sitemap.rs`
  or `site-tools/x.rs`) is still implementation.

## ADRs

- ADR-0004: plan-first implementation workflow.
- ADR-0005: repair-forward failure policy.

## Public API / CLI

None.

## Compatibility analysis

Only widens the set of non-implementation paths to the `site/` directory.

## Dependency analysis

No dependency changes.

## Expected file boundary

Plan checkpoint (Draft, for review):

- `.plans/P0-M003-R001-site-plan-exemption.plan.md`

Approval checkpoint:

- `.plans/P0-M003-R001-site-plan-exemption.plan.md` (`Status: Approved`)
- `.plans/ACTIVE`

Repair checkpoint:

- `tools/xtask/src/main.rs`: `is_implementation_path` and its unit test

Closure checkpoint:

- `.plans/ACTIVE` (removed)
- `.plans/P0-M003-R001-site-plan-exemption.plan.md` (Complete)

## Test-first matrix

| Path | Expected |
| --- | --- |
| `site/index.html` | not implementation |
| `site/script.js` | not implementation |
| `site/styles.css` | not implementation |
| `sitemap.rs` | implementation |
| `site-tools/build.rs` | implementation |
| `crates/agentforge-core/src/lib.rs` | implementation (unchanged) |
| `.github/workflows/pages.yml` | implementation (unchanged) |
| `docs/CI.md`, `.plans/ACTIVE` | not implementation (unchanged) |

## Implementation sequence

1. Commit this plan as Draft for review.
2. After the owner approves, a separate commit sets `Status: Approved` and
   points `.plans/ACTIVE` at this plan.
3. Add the test cases above, then change `is_implementation_path` to also
   return `false` for paths starting with `site/`.
4. Run the quality gates on the repair head.
5. Mark this plan Complete and remove `.plans/ACTIVE`.
6. Require post-merge CI green on `main`.

## Failure modes

- Matching on a bare `site` prefix and exempting real code such as `sitemap.rs`.
- Combining plan approval with the repair commit.
- Starting the repair before the Approved plan is on `main`.

## Documentation impact

Plan only. If `docs/CI.md` or `.plans/README.md` describes the classifier,
add one line noting the `site/` exemption.

## Quality gates

- `./scripts/check-text-files`
- `cargo run -p xtask --locked -- validate`
- `cargo run -p xtask --locked -- validate-plan-policy`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `./scripts/gate.sh full`

## Acceptance criteria

- [ ] `site/` changes pass the Repository policy job
- [ ] all other classifications unchanged, proven by unit tests
- [ ] `main` CI green after the closure checkpoint

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
