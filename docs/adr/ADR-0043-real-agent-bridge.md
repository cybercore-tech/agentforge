# ADR-0043: Real agents run through a bridge that owns path verification and commits

- Status: Accepted
- Date: 2026-09-24
- Milestone: P2-M027

## Context

Every AgentForge run before P2-M027 used fixture programs. Real coding agents need natural-language
instructions, not the length-delimited v1 stdin document, and a task's changes only reach review and
integration once they are committed on the task branch. The adapter deliberately provides no
filesystem sandbox, and an agent that commits itself could commit outside its boundary.

## Decision

- A per-agent bridge (first: `scripts/agents/claude-code-bridge`) sits between the process adapter
  and the agent. It decodes the prompt strictly and translates it into instructions.
- The agent never commits. The bridge verifies every changed path against the task's allowed and
  forbidden paths after the agent exits, then commits with the agent's detailed message plus task
  trailers. The repository's own pre-commit hook and gate run on that commit.
- Violations and gate failures leave the worktree uncommitted for review, with distinct exit codes.
- The adapter contract and trust boundary are unchanged.

## Consequences

Positive:

- a real agent's work flows through gates, review, accept, and integrate like any other task;
- boundary violations can never reach the task branch;
- other agents can reuse the decoder and commit logic.

Trade-offs:

- the path check is after the fact, so an agent can still write outside its boundary in the worktree
  (it is never committed);
- the agent's permissions (tool allowlist) are a bridge policy, not enforced by AgentForge itself;
- the pre-commit gate and the project gates both run, which is slower but gives independent evidence.
