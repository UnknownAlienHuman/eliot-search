# Assignment tickets

This directory is reserved for integration-owned immutable assignment tickets. Writer leases are separate
records under `swarm/leases/` and are derived only after a valid ticket and materialized context exist.

Canonical layout:

```text
swarm/tickets/<package>/<ticket_id>.toml
```

A ticket binds the package and stage, distinct writer/reviewer identities, immutable base commit, exact
write scope and feature profile, one materialized context, instruction digests, accepted dependency
handoffs, fixtures, bounded commands/evidence, unavailable checks and line limits. It follows
`swarm/schemas/assignment-ticket-v1.toml`, `swarm/orchestration.toml` and
`swarm/RECEIPT_CANONICALIZATION.md`.

Rules:

- package writers do not create or edit tickets;
- a ticket does not itself create a lease or authorize implementation before lease acknowledgement;
- one package has at most one active non-superseded lease;
- a changed base, context, assignment, instruction or dependency digest requires a new ticket;
- old tickets remain append-only historical records;
- ticket presence alone does not authorize work unless `swarm/launch-state.toml` authorizes the package;
- tickets contain no source bodies, secrets, raw queries or vendor credentials.
