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

Required quality gates name project gate profiles (`.forge/gates/<name>.conf`). A task that
declares required gates runs exactly those gates after a successful agent, each once, in lexical
gate-ID order. A task that declares none runs every project gate. A required gate with no profile
fails launch preflight, naming the gate, before the task becomes `running` or its agent starts.
See [GATES.md](GATES.md).

Task authority comes from the orchestrator/operator that creates the task.

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

## Approvals need the capability that uses them

Since P2-M035, creating a task (`forge task create`, or guided intake) refuses a post-execution
approval the task could never use:

- `merge_protected_branch` approval requires the `merge_protected_branch` capability, which
  `forge task integrate` checks;
- `deploy_production` approval requires the `deploy_production` capability.

The refusal names the flag to add (`add --capability merge_protected_branch`). Before this, such a
task could be accepted and approved, and then `forge task integrate` refused it, with no way to
amend the contract (dogfooding finding 21). `publish_release` and the pre-execution approvals have
no matching capability and are unaffected. The rule applies when a task is created; stored tasks
still load.
