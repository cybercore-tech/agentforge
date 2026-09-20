# ADR-0023: Controlled operator actions

Status: Accepted

## Decision

Operator mutations are exposed through explicit `forge task` commands backed by a focused
`agentforge-operator` service. The HUD and watch mode remain read-only. Inspection is read-only;
approval, acceptance, cancellation, and retry require explicit syntax and a bounded actor identity.

Approvals are append-only, task-linked `ApprovalRecorded` audit events using the existing stable
approval-boundary names. `forge run` accepts only approval evidence from a verified audit chain.
Lifecycle actions use `TaskGraph::transition` and append `TaskTransition` evidence.

## Rationale

Separating mutation from the HUD prevents a refresh surface from becoming an implicit authority
channel. Reusing core transition validation and the existing audit chain keeps human decisions
durable, reviewable, and provider-neutral.

## Consequences

Operators must identify themselves and name the exact task and approval boundary. Missing or corrupt
durable sources fail closed. Approval revocation, merge, deployment, and automatic repair remain
future decisions rather than hidden side effects.
