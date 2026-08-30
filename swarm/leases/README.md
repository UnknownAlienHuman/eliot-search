# Writer leases

Canonical layout:

```text
swarm/leases/<package>/<lease-id>.toml
```

Leases are integration-owned append-only records derived from an issued ticket and materialized context.
One package has at most one active non-superseded lease. A lease grants only the exact package write
scope at one base commit and stage; it cannot accept a package, select a provider or advance launch
state.

Use `swarm/WRITER_LEASE_TEMPLATE.md`. This directory currently contains no active lease.
