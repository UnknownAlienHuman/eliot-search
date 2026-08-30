# Writer leases

Canonical lease layout:

```text
swarm/leases/<package>/<lease_id>.toml
```

Append-only lifecycle events use:

```text
swarm/leases/<package>/events/<event_id>.toml
```

Leases are integration-owned immutable records derived from an issued ticket and materialized context.
One package has at most one active non-superseded lease. A lease grants only the exact package write scope
at one base commit and stage; it cannot accept a package, select a provider or advance launch state.

A lease has no implicit or automatic expiry. Implementation begins only after the assigned writer creates
an `ACKNOWLEDGED` lease event bound to the exact ticket, context and lease identities. Revocation,
submission and replacement are also append-only events with closed event/reason mappings.

Use `swarm/WRITER_LEASE_TEMPLATE.md`, `swarm/schemas/writer-lease-v1.toml` and
`swarm/schemas/lease-event-v1.toml`. This directory currently contains no active lease or lifecycle event.
