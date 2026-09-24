# ADR-0037: Orchestrated gate evidence

- Status: Accepted
- Date: 2026-09-23
- Milestone: P1-M004

## Context

P0-M007 built a bounded gate engine, but no crate used it. The orchestrator only accepted
caller-supplied `SliceEvidence::gates_passed`, and the persisted run and launch paths never ran a
gate. The audit schema defined `GateFinished`, and nothing emitted it. The spec's Phase 0 success
condition, "run deterministic gates", was true only for library composition.

## Decision

Gates are project configuration, not caller evidence. `agentforge-gate::GateProfileStore` loads
reviewed `.forge/gates/<id>.conf` profiles in the agent-profile format. The orchestrator:

- validates all gate profiles before the task becomes `running`;
- runs every gate, in lexical order, in the task's verified worktree after an agent exits with
  status zero;
- appends one `GateFinished` event per gate in the same attempt log as the agent evidence; and
- transitions the task to `failed` and records `FailureClassified` (`stage=gates`) when any gate
  does not pass.

All gates run even after one fails, so the operator sees the full evidence. The runner's
cleared-environment, direct-argument, and bounded-output rules are unchanged.

## Consequences

Positive:

- every operator run path produces real gate evidence without extra commands;
- a task whose gates fail can never be accepted without an explicit retry;
- gate configuration errors fail closed before any side effect.

Trade-offs:

- gates run serially and add their runtime to every task run;
- gate profiles must declare `PATH`, `HOME`, and similar values that toolchains need;
- gates are skipped after a non-zero agent exit, so that result carries no gate evidence;
- the legacy `SliceEvidence` API still accepts caller-supplied evidence for library callers.
