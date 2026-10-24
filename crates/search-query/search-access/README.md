# search-access — scope resolver, not a security operating system

The local product needs one operation: resolve a requested subset of the current `PublishedView` without
widening it. Restricted clients may add a real session restriction at their connection boundary.

## Current source

`src/lib.rs` is about 34.6 KiB and combines:

- signed/pairing-verified grant claims;
- recipe/modality/budget permissions;
- caller-shaped access snapshots and digests;
- per-membership eligibility plans;
- overlap proofs and singleton retrieval legs;
- live security/purge/shadow generations and per-phase permits.

`src/barrier.rs` is another 17.5 KiB durable restrictive-mutation coordinator with commit/readback,
published deny snapshots, dependent receipts and recovery phases.

The normal daemon path does not obtain authority from these structures. Public continuation/expansion
instead calls `LiveExpansionBarrier::clean()`, demonstrating that the machinery is disconnected rather
than protective.

## Target `ResolvedScope`

```text
ResolvedScope
  pinned PublishedView reference
  ordered selected source memberships
  projection memberships / stable filter fields needed by Qdrant
  exact source inventory references needed by Exact
  optional disclosure limits from a real restricted session
```

For the owning local OS account:

1. absent/default scope selects configured sources in the current view;
2. an explicit source set must be a subset of that view;
3. unknown/foreign IDs are rejected and an explicit empty set never widens to all sources;
4. one compatible selection becomes one Qdrant retrieval filter and the same `params.idf.corpus`;
5. exact scan uses the same selected view inventory;
6. source add/remove/policy change publishes a new view; an already pinned local query may finish on its
   old immutable view.

No grant, recipe permission, overlap certificate, predicate digest, purge generation or live checkpoint is
required for that path.

For a selected restricted-client profile, authenticate the connection and derive one allowed source set,
disclosure limits and cancellation owner. Each request must stay inside that set. Revocation closes the
session, cancels its in-flight work and removes its query windows. Do not enumerate every internal
request/leg/handle into a durable influence manifest or turn session revocation into a general mutation
journal.

## Population and routes

Compatible memberships share one retrieval/IDF population. Split only for different physical routes or
incompatible retrieval settings, then perform one bounded merge. Membership count alone does not create
one request per source. Do not duplicate Qdrant points to model overlapping selections.

The published view binds route/generation/profile. Query-only topical filters may narrow returned hits but
must not silently redefine the selected IDF corpus. An empty selected corpus remains empty.

## Removal

Change daemon admission, executor and bridge filter consumers together, then remove:

- local grant/template/recipe checks;
- `AuthoritativeAccessSnapshot`, `AuthorizedScope` proof fields and local snapshot digests;
- `BaseEligibilityPlan`, `EligibilityPlanDigest`, `OverlapFreeRouteProof` and per-member safe legs;
- `RequestSecurityFence`, `AccessPermit`, phase checkpoints and `LiveExpansionBarrier::clean()`;
- `SecurityMutationBarrier`, durable restriction receipts and purge/shadow generations from baseline;
- source-shape/proof fixtures for those APIs.

Keep root/source safety, explicit selected scope, real restricted-session checks and exclusion before
retrieval/statistics. If the surviving resolver is small, place it with query/view composition instead of
preserving this package boundary.

[FUNCTIONS.md](FUNCTIONS.md) maps current APIs and the exact cutover. [AGENTS.md](AGENTS.md) governs work.
