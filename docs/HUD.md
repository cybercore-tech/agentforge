# Operator HUD

`forge hud <root>` is a read-only, deterministic snapshot of durable project artifacts. It reports
the validated project name, mission, and guideline version; task lifecycle counts; verified audit
record counts and bounded recent activity; and verified managed worktree health.

The command reads `.forge/blueprint.conf`, `.forge/guidelines.md`,
`.forge/state/tasks.snapshot`, `.forge/audit.log`, and Git worktree metadata through existing
crate boundaries. It never creates `.forge` files, changes task state, launches an agent, or
mutates Git. Missing or corrupt sources fail closed with a source-labelled diagnostic. A missing
task snapshot is reported as unavailable rather than replaced with an empty graph.

Output is plain text with stable section and task-ID ordering. Recent audit activity is capped and
the complete report is bounded, so the snapshot can support a future interactive HUD without
becoming a second source of truth.

## Agent runs

The `agent_runs:` section follows `audit_recent` and lists the five most recent agent runs, oldest
first, one line per `AgentFinished` audit event:

```text
agent_runs:
  - #12 task=P1-M007-T0003 agent-exit=0 termination=exited gates=1/1 stdout=.forge/evidence/P1-M007-T0003/12-stdout.log stderr=.forge/evidence/P1-M007-T0003/12-stderr.log
  - #20 task=P1-M007-T0004 agent-exit=3 termination=exited gates=0/1 failed-gate=workspace:failed stdout=... stderr=...
```

- `agent-exit` is the recorded exit code (`none` when the platform reported none). Runs recorded
  before P2-M029, which carry no exit fields, show `agent-exit=unknown`.
- `duration=<seconds>s` is the time from the run's `AgentStarted` to its `AgentFinished`, shown when
  both are timestamped (P0-M015). Older runs have no duration. The recent events in
  `audit_recent` show `at=YYYY-MM-DDTHH:MM:SSZ` (UTC) for the same reason.
- `termination` is `exited`, `timed_out`, or `output_limit_exceeded`; `output-truncated=true`
  appears when the captured output was cut short.
- `gates=<passed>/<total>` counts the `GateFinished` events for the same task that follow the run,
  up to that task's next `AgentStarted`. `failed-gate=<gate>:<outcome>` names the first gate that
  did not pass.
- `stdout` and `stderr` are the project-relative evidence logs holding the agent's full output.
  When they could not be written, `evidence-error` gives the reason instead.

With no recorded runs the section reads `  - none`. The HUD shows evidence paths only and never
reads log contents; each field value is limited to one line of 256 characters.

## Watch mode

For a live read-only view, run:

```text
forge hud <root> --watch [--interval-ms <milliseconds>]
```

The default interval is one second; values are bounded to 50–60,000 milliseconds. Each frame is a
fresh source projection. Enter `r` or `refresh` to refresh immediately, `h` or `help` for the
command summary, and `q` or `quit` to exit. Input is ordinary line-oriented terminal input, so
redirected output remains plain text and no raw-terminal mode is required.

Source failures are printed as bounded diagnostics for the affected frame. The loop remains
read-only and may recover when a later frame becomes valid; it never creates missing durable state.

Operator mutations are deliberately separate from the HUD. Use explicit `forge task inspect`,
`approve`, `accept`, `cancel`, and `retry` commands when a human decision is required; each action
is validated and recorded in the durable audit chain.
