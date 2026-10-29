# Scope resolution — one pinned view, two filter levels

The local product resolves a requested subset of one pinned `PublishedView`. Restricted clients may add a real
session restriction at their connection boundary. This is not a grant, portfolio-history or policy operating
system.

## `ResolvedScope`

A small value carries only current consumers' inputs:

```text
PublishedViewGuard
ordered selected source memberships
base_population_filter
candidate_filter
exact retained inventory references
optional disclosure/read limits from one real restricted session
```

The view already binds the exact physical Qdrant collection, retrieval profile and redb snapshot. Stage 2/3A has
no sequence or route label in scope.

## Selection semantics

1. Absent/default source selection resolves to the configured sources present in the pinned view.
2. Explicit source IDs must be a subset of that view and any real session-allowed set.
3. Unknown/foreign IDs fail. Explicit empty is empty and never becomes wildcard.
4. Incomplete/unavailable membership is a gap/error, not permission to widen.
5. Exact scans and evidence validation consume the same selected source/revision inventory.
6. A source registration/policy change publishes a new view; an already pinned local request may finish on the old
   immutable view.

No canonical scope digest, historical alias revision, overlap proof, membership receipt or retention edge is
required.

## Base population versus candidate narrowing

Keep these distinct:

```text
base population = selected source memberships ∩ access/session restriction
candidate filter = base population + explicit request-only path/kind/topical narrowing
idf.corpus       = base population
```

Source selection and access restriction affect both results and BM25 statistics. Request-only narrowing affects
candidate eligibility but does not silently redefine IDF. Every filter field is payload-indexed in the physical
collection schema.

If the base population is the whole physical collection, omit `idf.corpus`; Qdrant's global statistics are then
exactly the selected population. Stage 2's one-file smoke uses this simplest form. Known empty returns empty without
RPC.

Compatible sources in one physical collection use one `TextQuery`. Different collections or incompatible
retrieval profiles remain separate populations and cannot be combined by sorting raw scores. Add a bounded merge
only for a real multi-population product operation.

Qdrant BM25 statistics are physical-shard-local. Do not partition a general portfolio by source/root shard key and
then pretend scores are globally comparable. Custom sharding is only reconsidered for a scope always queried as one
partition.

## Restricted session

A selected restricted-client profile authenticates one connection and derives one allowed source set, disclosure
limits and cancellation owner. Requests only narrow that set. Revocation closes the session, cancels its work and
removes its windows. No durable influence manifest, phase permit or restriction-mutation journal.

## Removal

Change query, Exact, evidence and bridge consumers together, then remove caller-created grants, recipes,
`AuthoritativeAccessSnapshot`, eligibility plans/digests, overlap proofs, phase checkpoints, purge/shadow
generations and the disconnected barrier platform. If the resolver becomes a few pure functions, place it in query
composition instead of preserving this crate.

## Focused checks

Verify default/explicit/empty/unknown source sets; restricted narrowing; base filter appearing in both candidates
and `idf.corpus`; request-only filter absent from `idf.corpus`; whole-collection omission of redundant predicates;
excluded-source noninterference; incompatible-profile split; old pinned view after source update; and exact/evidence
inventory agreement. R051 adds no runtime result.
