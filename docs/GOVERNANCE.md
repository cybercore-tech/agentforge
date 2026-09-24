# AgentForge Governance

AgentForge uses human-controlled, least-privilege agent execution.

## Governing principles

1. Models are replaceable workers.
2. Roles describe responsibility.
3. Capabilities describe authority.
4. Tasks describe scope.
5. Plans describe intended change.
6. Gates provide evidence.
7. Humans approve consequential boundaries.
8. Results report what happened but cannot self-authorize future actions.

## Reviewer independence

Reviewers are read-oriented by default.

When a reviewer discovers a defect, the normal output is a finding or repair task. A reviewer does
not silently transform itself into the implementer whose work it is reviewing.

## Concurrent work

Write-capable tasks receive non-overlapping ownership by default.

Shared generated files or integration surfaces require serialization or an explicitly assigned
Integrator.

Read-only inspection may occur concurrently.

Dirty working trees are never shared between agents.

## Hermetic gate

`./scripts/gate.sh` runs from the pre-commit hook, and that hook also runs inside AgentForge task
worktrees whenever an agent's work is committed. Since P2-M028 the gate is hermetic there:

- Before the cargo steps it clears every `GIT_*` variable. In a linked worktree, Git gives hooks an
  absolute `GIT_DIR` and `GIT_INDEX_FILE` pointing at the real repository, and test fixtures that
  shell out to `git` would otherwise act on it. This once set `core.bare = true` in the shared
  config, injected a fake `[user]`, and committed a fixture onto a task branch. The plan-policy
  check runs before the variables are cleared, because it must read the hook's index.
- In a linked worktree it builds into `<target dir>/agentforge-worktrees/<worktree name>`. Cargo
  names workspace-member artifacts independently of the checkout path, so checkouts sharing a
  target directory would otherwise run each other's binaries as "fresh".

If a gate in a task worktree ever misbehaves, check the main checkout's `.git/config`
(`core.bare`, `[user]`) and the task branch history before anything else.

## Milestone tags

Every completed milestone is marked with an annotated `milestone/<ID>` tag on its closure commit
(ADR-0044). Tagging is the last step of a closure, after the closure commit is pushed and green.
Milestone tags are permanent project history: they are never moved or deleted, and they are kept
separate from `vX.Y.Z` release tags.

## Escalation

An agent must stop and request escalation when:

- required work exceeds its allowed paths;
- required capability was not granted;
- a human approval boundary is reached;
- a destructive or irreversible operation becomes necessary;
- project state conflicts with the task contract;
- evidence is insufficient to make a safe decision.

Escalation is a valid task outcome, not an agent failure.
