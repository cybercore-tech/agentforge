# Gates

P0-M007 represents a local check as an explicit `GateDefinition`: a stable name, absolute
executable path, literal arguments, explicit environment values, deadline, and shared output limit.

`GateRunner` clears the child environment, executes the argument vector directly, captures raw
stdout and stderr concurrently, and reports whether the child passed, failed, timed out, or exceeded
its output budget. A report is evidence only; it never accepts a task or initiates repair.

Batch reports retain definition order and duplicate names are rejected before any process starts.
