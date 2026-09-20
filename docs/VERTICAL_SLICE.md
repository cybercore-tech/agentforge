# Single-agent vertical slice

`agentforge-orchestrator` composes existing contracts in one ordered, fail-closed flow:
policy validation, worktree verification evidence, bounded agent evidence, gate evidence, and
review handoff. It accepts no task, performs no automatic transition, and does not schedule peers,
merge branches, deploy, or repair failures. Callers own actual side effects and audit persistence.
