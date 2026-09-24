# Gates

P0-M007 represents a local check as an explicit `GateDefinition`: a stable name, absolute
executable path, literal arguments, explicit environment values, deadline, and shared output limit.

`GateRunner` clears the child environment, executes the argument vector directly, captures raw
stdout and stderr concurrently, and reports whether the child passed, failed, timed out, or exceeded
its output budget. A report is evidence only; it never accepts a task or initiates repair.

Batch reports retain definition order and duplicate names are rejected before any process starts.

## Project gate profiles

P1-M004 makes gates part of every task run. A project declares gates as reviewed files in
`.forge/gates/<gate-id>.conf`, using the same bounded format as agent profiles:

```text
version=1
executable=/home/operator/.cargo/bin/cargo
argument=test
argument=--workspace
argument=--locked
env.PATH=/usr/bin:/bin
env.HOME=/home/operator
timeout_ms=600000
max_output_bytes=1048576
```

The file stem is the gate name (1-64 ASCII letters, digits, `-`, or `_`). The executable must be
an existing absolute file. Arguments are literal, no shell is invoked, and the gate receives a
cleared environment plus only its `env.` values, so tools that need `PATH` or `HOME` must declare
them. Profiles are bounded to 64 KiB, 64 arguments, 64 environment values, a 24-hour timeout, and
a 64 MiB output limit. Unknown or repeated keys, symlinked files, and out-of-range values fail
closed. `forge gate list <root>` loads and prints every profile without running anything.

## Orchestrated execution

`forge run`, `forge task launch`, `forge daemon run`, and `forge daemon launch` all use the same
orchestrated path:

1. All gate profiles are loaded and validated before the task becomes `running`. One malformed
   profile stops the run before the agent starts, with no state or audit change.
2. The task's gates are selected (P1-M007). A task with an empty `required_gates` list selects
   every project gate. Otherwise it selects exactly the named gates; a name listed twice selects
   the gate once. A required gate with no `.forge/gates/<name>.conf` profile fails preflight with
   `required gate is not configured: <name>` (the first missing name in lexical order), before the
   task becomes `running`, before a worktree is created or observed, and before the agent starts.
3. The agent runs as before.
4. If the agent exited normally with status zero, every selected gate runs in lexical ID order in
   the task's verified worktree. If the agent exited non-zero, timed out, or hit its output limit,
   gates are skipped; that result is already the evidence.
5. Each selected gate appends a `GateFinished` audit event with `gate`, `outcome` (`passed`,
   `failed`, `timed_out`, `output_limit_exceeded`, or `error`), `exit_code`, and
   `output_truncated` (or `error`) fields.
6. If any gate did not pass, the task transitions to `failed` and a `FailureClassified` event
   records `stage=gates` and the first failing gate. The CLI prints one line per gate plus a
   `gates=<passed>/<total>` summary and exits 1. Daemon responses include the same summary.
7. When every selected gate passes, the task stays `running`. Acceptance is still an explicit
   operator decision, and only a task that passed its gates can reach it.

`forge task launch-batch` applies the same selection to each task during its prepare phase. A task
whose required gate is not configured is reported as skipped with the same reason, keeps its
`pending` state and has no worktree created; its siblings still launch.

Tasks built from a blueprint only inherit default gates that have a profile; explicit `--gate`
names are kept as given and are checked here (see [BLUEPRINT.md](BLUEPRINT.md)).

Gates should be read-only checks. A gate that writes tracked files leaves the worktree dirty, and
retirement will refuse it, as with any other dirty worktree.

A project with no `.forge/gates/` directory runs exactly as before, with a `gates=0/0` summary.
