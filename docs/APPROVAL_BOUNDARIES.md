# AgentForge Human Approval Boundaries

Human approval is authoritative.

The default repository-wide boundaries are:

- activating an implementation plan;
- expanding task scope beyond approved ownership;
- adding or materially changing third-party dependencies;
- capability or privilege elevation;
- access to secrets;
- destructive data migration;
- irreversible external state changes;
- merging into a protected branch;
- publishing a release;
- production deployment;
- changing the governance rules that define approval boundaries.

Projects may require additional approvals.

An unrelated implementation task must never weaken these boundaries as a side effect.
