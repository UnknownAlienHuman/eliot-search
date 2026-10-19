# Restricted scope, revocation and restore

The old W7 invalidation-only, purge and restore publication protocol is not a baseline product subsystem.
It mixed access control, legal retention, Qdrant cleanup and publication recovery into another state machine.

## Local baseline

The owning-account local application resolves an explicit non-widening `ResolvedScope` from the pinned
`PublishedView`. It does not persist a security command, deny generation, purge receipt or invalidation-only
view sequence for every publication.

## Optional restricted client

A selected restricted-client adapter owns its real session policy:

- explicit allowed source/view set;
- non-widening retrieval filter and identical scoring/IDF population;
- request limits, expiry and revocation;
- manifest/evidence lookup constrained to the same scope.

Revocation changes the adapter/session's admitted scope immediately. Physical Qdrant bytes are not access
authority. Ordinary publication can later remove or replace points; cleanup waits for view guards where
required.

A pending publication that overlaps a newly restricted source does not need epoch abandonment,
compensation or a membership fence. Queries cannot widen into the source; the same pending operation is
resolved from actual Qdrant/current-view state, and the next ordinary publication reflects the new selected
source set.

## Restore

A restored Qdrant backup or collection is merely candidate external data. Validate it against current
source/preparation manifests, publish it as a normal new physical generation and commit one new
`CurrentPublishedView`. An alias or historical epoch never makes it current by itself.

## Deliberately absent

Do not implement invalidation-only publication, purge tombstones, security-purge target/receipt families,
consumed abandoned epochs, restore-candidate quarantine or a second access snapshot publisher. A real legal
hold, secure erasure or multi-tenant authorization feature may introduce its own selected requirements only
when an actual consumer exists; it must not burden the baseline local path.

See [FUNCTIONS.md](FUNCTIONS.md),
[`search-access`](../../search-query/search-access/README.md) and the
[view-guard map](../search-epoch-pins/FUNCTIONS.md).
