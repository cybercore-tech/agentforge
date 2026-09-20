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
