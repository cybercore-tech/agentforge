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

The normal task preflight still applies: the task must exist, its managed worktree must be clean
and verified, and required approvals and capabilities must already be present.
