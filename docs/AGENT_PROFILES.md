# Local agent profiles

AgentForge can keep a reviewed, project-local process profile under
`.forge/agents/`. Profiles are ordinary versioned text files. They describe only how
to start a local executable; they do not grant task capabilities, approvals, or repository access.

Each profile is named `<id>.conf` and uses one `key=value` field per line:

```text
version=1
executable=/absolute/path/to/agent
argument=--mode
argument=worker
env.FORGE_MODE=local
timeout_ms=60000
max_output_bytes=1048576
```

The supported environment form is `env.<NAME>=<VALUE>`; the child receives a cleared
environment plus only these explicit values. Arguments and values are literal strings. No shell is
invoked and no interpolation is performed.

Profiles are bounded to 64 KiB, 64 arguments, 64 environment values, a 24-hour timeout, and a
64 MiB output limit. The executable must be an existing absolute regular file. Duplicate fields,
unsafe profile IDs, malformed values, symlinked profile files, and out-of-range limits fail closed.

Inspect profiles without launching anything:

```text
forge agent list /path/to/project
forge agent validate /path/to/project local-agent
forge agent inspect /path/to/project local-agent
```

Select one for direct or daemon-backed execution:

```text
forge run /path/to/project P2-M007-T0001 --profile local-agent
forge daemon run /path/to/project P2-M007-T0001 --profile local-agent
```

For a direct foreground session where the operator must answer line-oriented agent questions, add
`--interactive` to `forge run`. The profile's literal executable, arguments, explicit environment,
timeouts, and evidence bounds remain in force; `forge daemon run` does not attach a terminal.

For a full-screen or raw-mode agent, add `--interactive --pty` instead. This allocates a native
pseudo-terminal, forwards keyboard input and terminal resize events, and restores the operator's
terminal mode when the child exits. PTY mode is intentionally direct-foreground only and requires
both stdin and stdout to be terminals; it fails closed in CI pipes and daemon execution.

The normal task preflight still applies: the task must exist, its managed worktree must be clean
and verified, and required approvals and capabilities must already be present.

## Real agents: the Claude Code bridge

Real coding agents read natural-language instructions, not the length-delimited
`agentforge-task-prompt-v1` document, and a task's changes only reach `forge task diff` and
`forge task integrate` once they are committed on the task branch.
`scripts/agents/claude-code-bridge` (Python 3, standard library) closes that gap for Claude Code:

1. It strictly decodes the task prompt from stdin; malformed input exits 2 before any agent runs.
2. It runs `claude -p` headless in the task worktree. The instructions cover the goal, non-goals,
   allowed and forbidden paths, expected outputs, evidence, gates, and the repository's
   `AGENTS.md`. Permissions are `acceptEdits` plus a bounded tool allowlist: read/edit tools,
   `cargo`, `./scripts/gate.sh`, read-only `git`, and `ls`. `git commit`/`push`/`reset`/`checkout`/
   `clean` and web tools are denied.
3. It lists every changed path and refuses (exit 4, nothing committed, worktree kept) if any path
   is outside the task's allowed paths or inside a forbidden one.
4. It commits the changes itself, using the detailed message the agent writes to a scratch file,
   plus `AgentForge-Task`/`AgentForge-Agent` trailers. The repository's pre-commit hook runs on
   that commit, and a hook or gate failure exits 5.

Exit codes: 0 committed, 2 invalid input, 3 no changes, 4 path violation, 5 commit/gate failure;
anything else is Claude Code's own exit status. As with any adapter, exit 0 is evidence, not
acceptance: project gates, review, and `forge task accept` still follow.

Profile (the environment is cleared, so pass what Claude Code and your toolchain need):

```text
# .forge/agents/claude-code.conf
version=1
executable=/usr/bin/python3
argument=/path/to/agentforge/scripts/agents/claude-code-bridge
argument=--claude
argument=/absolute/path/to/claude
env.PATH=/usr/bin:/bin:/home/operator/.local/bin
env.HOME=/home/operator
timeout_ms=3600000
max_output_bytes=8388608
```

Add `argument=--model` and `argument=<model>` to pin a model. Use `--dry-run` (reads a prompt on
stdin and prints the instructions) to preview what the agent will be told, and `--self-test` (also
run in CI) to check the decoder and path rules. The path check runs after the agent, so it is a
commit guard, not a sandbox. See [`DOGFOODING.md`](DOGFOODING.md) for a full run on AgentForge
itself.
