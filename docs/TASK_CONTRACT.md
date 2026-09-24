# AgentForge Task and Result Contract

P0-M002 defines contract semantics independent of serialization.

## AgentTask

Every executable agent task carries:

- contract version;
- task ID;
- milestone ID;
- primary role;
- goal;
- non-goals;
- dependency task IDs;
- allowed paths;
- forbidden paths;
- capability grants;
- required human approvals;
- required quality gates;
- expected outputs;
- evidence requirements.

Task authority comes from the orchestrator/operator that creates the task.

Required quality gates name project gate profiles in `.forge/gates/`. Since P1-M007 they select
which gates run after a successful agent: exactly the named gates, each once, in lexical order.
A task with no required gates runs every project gate. A named gate with no profile fails
preflight before the task starts. `docs/GATES.md` records the full rule.

The worker may request escalation but may not rewrite its own authority.

## AgentResult

Every completed or interrupted execution reports:

- contract version;
- originating task ID;
- outcome;
- summary;
- changed paths;
- commit identity when applicable;
- gates executed;
- tests added or changed;
- evidence produced;
- unresolved risks;
- requested escalation;
- handoff notes.

## Outcomes

Initial semantic outcomes are:

- Completed
- Blocked
- EscalationRequired
- Failed

A failure is not automatically permission to retry with wider scope.

## Versioning

Contracts carry an explicit version from their first implementation.

P0-M002 does not stabilize TOML, JSON, protobuf, or any other wire/storage encoding. Later
milestones may define encodings around the provider-neutral semantic contract.
