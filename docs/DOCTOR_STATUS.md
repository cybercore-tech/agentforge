# Doctor and status diagnostics

`forge doctor` and `forge status` are bounded, read-only observations. They report repository
readiness, active plan pointers, project-state presence, and known diagnostic limits in stable line
order. Missing files are findings rather than panics. These commands never repair state, mutate
worktrees, transition tasks, launch agents, or inspect secrets.
