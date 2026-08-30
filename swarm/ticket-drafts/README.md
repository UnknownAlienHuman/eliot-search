# Non-claimable assignment-ticket drafts

Canonical layout:

```text
swarm/ticket-drafts/<stage>/<package>.toml
```

A schema-v2 draft predefines package/stage/write/output/evidence boundaries but deliberately leaves the
writer, reviewer, immutable base commit, materialized context, accepted dependency handoffs and ticket
identities unresolved.

The unresolved ticket identity has two distinct digest slots:

```text
ticket_signed_payload_sha256
ticket_exact_record_file_sha256
```

Neither is a complete-file self-hash embedded in the future ticket. The draft contains no `lease_id`:
lease issuance is a separate operation after exact assignment-ticket readback.

A draft:

- is not copied verbatim into `swarm/tickets/`;
- never authorizes implementation;
- never creates or identifies a writer lease;
- cannot be acknowledged by an agent;
- may describe current launch classification and unmet issuance prerequisites;
- requires separate context materialization, ticket issuance, lease issuance and append-only writer
  acknowledgement operations;
- is resolved only by integration-owned operations that create new immutable records.

Any source/registry/context/base/dependency change requires a newly materialized context and ticket.
