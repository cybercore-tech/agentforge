# Scheduling and serialized integration

P1-M001 introduces deterministic runnable batches from the durable task graph. Tasks are ordered by
stable ID and only disjoint owned paths may share a batch. Overlapping ownership is rejected
conservatively. Integrator reservations are single-owner and serialized. Scheduling produces evidence
and reservations; it does not accept results, grant capabilities, merge protected branches, or deploy.
