# Assignment tickets

This directory is reserved for integration-owned assignment and writer-lease records.

Canonical layout:

```text
swarm/tickets/<package>/<ticket-id>.toml
```

A ticket binds the package, writer, lease, base commit, exact write scope, assignment/instruction
digests, accepted dependency commits/API digests, feature profile and required evidence. It follows
`swarm/ASSIGNMENT_TICKET_TEMPLATE.md`, `swarm/orchestration.toml` and
`swarm/RECEIPT_CANONICALIZATION.md`.

Rules:

- package writers do not create or edit tickets;
- one package has at most one active non-superseded lease;
- a changed base, assignment or dependency digest requires a new ticket;
- old tickets remain append-only historical records;
- ticket presence alone does not authorize work unless `swarm/launch-state.toml` authorizes the package;
- tickets contain no source bodies, secrets, raw queries or vendor credentials.
