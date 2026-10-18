# search-point-identity

Derive one stable retrieval-point identity and its 128-bit Qdrant address. The package is stateless and
contains no registry, source authority, transport or publication state.

## Current source

`src/s11.rs` implements an eight-coordinate v1 key containing generation, projection membership,
representation, Unit ID, profile set and role. The full BLAKE3 digest is projected to a 128-bit address;
readback compares the address, full digest and coordinates. These collision/address mechanics are useful.

The key shape is not the target. Representation and current Unit ID are revision/coordinate-bearing in the
existing preparation path, so a local edit can mint a new point for every later unit. The
[current/target map](S11_PROFILE.md) describes the cutover.

## Target

Point identity contains only immutable retrieval state for one generation:

```text
projection membership / stable grouping
occurrence incarnation
retrieval profile and role
exact retrieval-input digest
point role
```

Exact source revision, representation, logical occurrence and range live in the published-view manifest.
Old pinned views retain their old evidence binding while an unchanged retrieval point can remain stored.
Equal text occurrences remain distinct; changed input/profile/incarnation creates a new point.

Change planner, payload, publication, bridge and query consumers together. New derived formats rebuild;
there is no old S11 compatibility/migration requirement. Remove the separate root legacy minting/registry
and obsolete goldens with their last callers.

[Implementation guide](../../../docs/implementation/README.md) · [Agent notes](AGENTS.md)
