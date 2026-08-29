# Agent contract — eliot-searchd

You own only `bins/eliot-searchd/`. This is a progressive composition package, not a capability
implementation. Read `swarm/assignments/eliot-searchd.md` and only the accepted dependency handoffs for
the active Cargo feature layer.

## Mission

Compose accepted capability crates, own the data root and expose the only storage/provider process
boundary while keeping `main` thin.

## Progressive layers

```text
wave1-shell       owner + journal + provider framing
wave2-source      direct source/revision/materialization spine
wave3-index       lexical/Qdrant/publication/pins
wave4-query       access/planner/executor/validation/cards/continuations
wave5-current     reconciliation/overlay/Rust structure
wave6-proof       exact scan/subject/comparison
wave7-lifecycle   retention/purge/restore hardening
```

The default feature is `wave1-shell`. A final dependency appearing in Cargo is not permission to read it
or enable it. Each later layer requires accepted public handoffs and integration-owner activation.

## Ownership

- dependency injection and startup order;
- data-root owner guard and sole Qdrant process supervision;
- bounded task/server lifecycle;
- readiness/degradation capability descriptor;
- controlled drain, cancellation and shutdown.

## Forbidden ownership

- capability logic inside `main`;
- direct dependency on unaccepted future-wave behavior;
- sharing redb/Qdrant clients with CLI, workers or adapters;
- a second data-root/index owner;
- hidden fallback across profiles.

## Write and integration boundary

The writer edits only this package. Root feature policy, workspace members, lockfile and launch state
belong to the integration owner. New behavior belongs in the package that owns its state/failure seam.

## Size guard

Target `src/` ≤ 6,500 lines. Mandatory split/design review occurs before 8,500 total hand-written Rust
lines; 10,000 including local tests is a hard stop.
