# Dogfooding: AgentForge building AgentForge

P2-M027 runs a real coding agent (Claude Code, through `scripts/agents/claude-code-bridge`) on this
repository through the normal AgentForge flow: task contract, managed worktree, agent, project
gates, review, accept, integrate. This file is the run log and the list of friction points found.

## Bridge verification (2026-09-24)

| Check | Result |
| --- | --- |
| `--self-test` | Decoder round-trips multiline/Unicode fields; rejects truncated, bad-header, bad-length, renamed-field, and trailing-byte prompts; path rules correct |
| `--dry-run` on a `forge`-rendered prompt (captured with a `tee` profile) | Complete instructions for every contract field |
| Real Claude Code on a scratch repository (`forge task launch --profile claude-code`) | Edited only the allowed file, wrote a detailed conventional commit message, bridge committed it with task trailers; 14 s end to end; worktree clean |
| Path guard with a fake agent that also writes outside the boundary | Exit 4, nothing committed, task branch still at its base, worktree left dirty for review |

## Findings

1. **A non-zero agent exit is nearly invisible.** `forge task launch` prints only
   `termination=Exited` and exits 0 when the bridge exits 4 (path violation). The evidence is
   recorded, but the operator is not told. The CLI should print the agent's exit code and exit
   non-zero (follow-up).
