# Plan: P2-M034 — Closure and documentation-index integrity

Status: Approved
Milestone: P2-M034
Created: 2026-09-24
Owner: AgentForge project

## Goal

Close two gaps where the process depended on someone remembering a step:

1. **Finding 15 — the tagger.** `scripts/tag-milestone` can no longer tag a milestone that was
   never closed. It decides only from committed history, refuses uncommitted closure changes, and
   never applies the legacy "last commit" fallback to a plan that uses status lines.
2. **The documentation indexes.** `docs/adr/README.md` lists only ADR-0001 to ADR-0025 (last
   updated 2026-09-20), so ADR-0026 to ADR-0051 are missing, seven of them from the most recent
   milestones. Four docs (`AGENT_ADAPTERS`, `AGENT_ROLES`, `DOCTOR_STATUS`, and `VERTICAL_SLICE`)
   are not linked from the README's documentation map. Both are fixed, and `xtask validate` (run
   by the gate and CI) now fails when any ADR is missing from the registry, a registry row has no
   file, or any top-level `docs/*.md` is not linked from the README.

## Non-goals

- No change to existing tags (all 69 must resolve to the same commits), the tag message format, or
  the push policy.
- No change to AGENTS.md rule 14. The dry-run subject check stays the operator's duty; the tagger
  becomes a second line of defence.
- No rewrite of ADR contents. Registry rows use each ADR's own title and status.

## Context

- **Finding 15.** At the P0-M014 closure, the closure commit was rejected by the text policy, and a
  pipe masked `git commit`'s exit code. The tagger read the working-tree `docs/MILESTONES.md`
  (already `complete`), found no committed `Status: Complete`, fell back to the legacy last-commit
  rule, and tagged and pushed the **approve** commit. It was deleted within about two minutes and
  recreated on the real closure.
- **The index gap.** When the operator asked whether the documentation workflow was holding, a
  check showed that per-change docs were always updated but the indexes were not. Nothing enforced
  them. It is the same class of problem as the release workflow's publish step: correctness that
  depends on a remembered step.

## Architecture placement

- **`scripts/tag-milestone`:**
  - `docs/MILESTONES.md`, the plan list (`git ls-tree HEAD .plans/`), and completion records are
    read from `HEAD`.
  - It refuses when `git status --porcelain` shows changes to `docs/MILESTONES.md` or the
    milestone's plans.
  - The legacy fallback applies only when no committed version of the plan ever had a `Status:`
    line; otherwise a plan without a committed `Status: Complete` is an error.
  - `--self-test` (already run in CI) gains throwaway-repo integration cases:
    - the P0-M014 replay;
    - a committed `complete` row with an unclosed plan;
    - a proper closure;
    - a legacy plan;
    - prose that mentions the status line.
- **`tools/xtask` (`validate`):**
  - `validate_adr_registry`: the ADR IDs from `docs/adr/ADR-*.md` must equal the `| ADR-NNNN |`
    rows in `docs/adr/README.md`;
  - `validate_docs_map`: every `docs/*.md` must be linked as `(docs/<name>.md)` in `README.md`;
  - pure functions, unit tested.
- **Docs:** 26 registry rows added (ADR-0026 to ADR-0051, with each title and status from its
  file), and four README links added.

## Invariants

- A milestone tag lands only on a committed closure (or a genuinely legacy plan's last commit), and
  never comes from uncommitted state. Existing tags are unchanged.
- Every ADR is in the registry, and every doc is reachable from the README, enforced by CI.

## ADRs

An ADR-0044 addendum (the tagger rules as of P2-M034).

## Public API / CLI

The same CLIs, with stricter refusals.

## Compatibility analysis

`tag-milestone --all --dry-run` must report every existing tag as `already tags` with the same
commit, before and after.

## Dependency analysis

None.

## Expected file boundary

- `.plans/P2-M034-closure-and-docs-integrity.plan.md`, `.plans/ACTIVE`
- `scripts/tag-milestone`, `tools/xtask/src/main.rs`
- `docs/adr/README.md`, `docs/adr/ADR-0044-milestone-tags.md`, `README.md`, `docs/OPERATIONS.md`,
  `docs/DOGFOODING.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `PROJECT_STATE.md`, `AGENT_HANDOFF.md`

## Test-first matrix

- Tagger: the P0-M014 replay fails on the current script first, then passes; the other integration
  cases; `--all --dry-run` gives 69 × unchanged.
- xtask: the unit tests (missing registry row, dangling row, unlinked doc, all consistent) fail
  against the current repository before the docs are fixed, and pass after.
- The full gate and CI (push plus a repeat).

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Baseline `--all --dry-run`. Tagger: the replay test first, then the fix. xtask checks: they
   first fail on the current docs (proving they bite), then the docs are fixed.
4. ADR addendum, OPERATIONS, and DOGFOODING; gate; commit; push; CI plus a repeat.
5. Close, with the commit exit code checked without a pipe; tag only after the dry run names the
   "close ..." commit.

## Failure modes

- A legacy plan misclassified: the `--all --dry-run` regression check catches it.
- A future doc added without a link or registry row: CI fails, as intended.

## Documentation impact

The ADR registry and README map (complete), the ADR-0044 addendum, OPERATIONS (Tags, and the new
validation), and DOGFOODING (finding 15 resolved).

## Quality gates

- `./scripts/gate.sh full`, `scripts/tag-milestone --self-test`, and `--all --dry-run`;
- push CI plus a dispatched repeat.

## Acceptance criteria

- [ ] The tagger decides only from `HEAD`, refuses uncommitted closures and unclosed plans, and
      keeps the legacy fallback for plans without status lines only; all 69 tags are unchanged.
- [ ] The ADR registry lists all 51 ADRs, the README links every doc, and `xtask validate`
      enforces both in CI.
- [ ] Docs updated; CI evidence; closed and tagged correctly.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
