# ADR-0038: Operator CI observation with recorded classification

- Status: Accepted
- Date: 2026-09-23
- Milestone: P1-M005

## Context

P0-M008 built an exact-SHA CI monitor and a conservative failure classifier, but no crate used
them. AGENTS.md requires every failed CI run to be classified before repair, and that step was done
by hand. The audit schema's `CiObserved` event was never emitted.

## Decision

Add `forge ci observe`, backed by `agentforge_operator::observe_ci`. The provider command comes
from a reviewed `.forge/ci/provider.conf` profile instead of command-line flags, so the command
that can reach the network and credentials is reviewed and fixed per project. One call records one
`CiObserved` event and one `FailureClassified` event (`stage=ci`) per failed job. It never changes
task state, retries, or repairs.

AgentForge still contains no network client or credentials. `scripts/ci-provider-github` is an
opt-in reference provider for GitHub Actions. It reports the most recent run when a workflow ran
more than once for one SHA, because the monitor rejects ambiguous evidence by design.

## Consequences

Positive:

- classification before repair is a recorded, auditable step that shows in the HUD;
- CI evidence can be linked to a task without giving CI any authority over it;
- the existing exact-SHA, bounded-output, and fail-closed protections apply unchanged.

Trade-offs:

- observation is a point-in-time snapshot; the operator re-runs it to see a later state;
- only `failure` conclusions are classified, so cancelled or timed-out runs are recorded without a
  category;
- the reference provider depends on `gh` and `python3`, and its excerpt heuristic is a best
  effort.
