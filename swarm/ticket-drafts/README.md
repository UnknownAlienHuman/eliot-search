# Non-claimable assignment-ticket drafts

Canonical layout:

```text
swarm/ticket-drafts/<stage>/<package>.toml
```

A draft predefines package/stage/write/output/evidence boundaries but deliberately omits the writer,
reviewer, base commit, materialized context, accepted dependency handoffs, ticket digest and lease.

A draft:

- is not copied verbatim into `swarm/tickets/`;
- never authorizes implementation;
- never creates a writer lease;
- cannot be acknowledged by an agent;
- may describe current launch classification and unmet issuance prerequisites;
- is resolved only by an integration-owner issuance operation that creates a new immutable ticket.

Any source/registry/context/base/dependency change requires a newly materialized context and ticket.
