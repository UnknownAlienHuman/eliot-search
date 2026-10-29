# Control state and evidence lookup

Redb stores compact current/published metadata and supplies real read snapshots. Immutable objects hold bodies;
Qdrant holds derived retrieval projections. This owner creates no generic journal, receipt history or second corpus.

## Typed state

```text
root/settings/current sources
current preparation outcomes
PendingGenerationBuild
published_sources[g]: source_id -> exact source/prepared refs + selected outcome/gap
published_evidence[g]: (source_id, point_id) -> complete identity + ObjectRef/representation/range
CurrentPublishedGeneration
RetiredCleanup
```

Rows are metadata, not source bodies, retrieval text or vectors. Qdrant routing payload nominates the key; the
pinned evidence row and actual retained reader establish evidence. Source-prefix diff reads are conditional Stage 3B.

## Durable binding versus process view

Durable current stores physical collection/profile and the completed generation's metadata binding. An owned redb
ReadTransaction is process-local; it is opened/verified after commit and retained by PublishedView, not serialized
as durable current. Old queries use that same transaction, never fresh latest metadata under an old view label.

Physical profile excludes request-time k/b. Query captures its separate scoring profile; changing only k/b does
not mutate current or rebuild. Stage 2/3A has no sequence or arbitrary historical-view lookup service.

Long read transactions retain pages. Bound request/window lifetime and publication backlog and measure actual
retained pages/collections; Arc count is not a byte bound. Last guard drop only wakes cleanup.

<a id="restartable-generation-input-capture"></a>
## Complete generation capture

1. Open one consistent read of selected source/preparation outcomes and physical profile.
2. Copy compact fixed refs/outcomes into target-generation source rows in bounded transactions.
3. Mark capture complete only after enumeration and validation; interruption beforehand discards that unserved
   metadata-only target and restarts capture without backend effects.
4. Derive all later pages and evidence from the complete captured input. Missing/corrupt refs fail, not substitute
   newer live state. Large uploadable pages use immutable objects, not oversized metadata rows.
5. Populate unserved target evidence rows in bounded transactions. Do not collect/copy the corpus in the final writer.

An in-memory read transaction alone cannot recover desired input after process death. Stage 2/3A pages use actual
retrieval_text and deterministic generation-bound IDs; no occurrence matching or validity interval.

## Publication commit

After complete backend readiness, including exact schema/point/count and TextQuery canary, and completed staging
metadata, one bounded transaction compares pending/current guards, switches CurrentPublishedGeneration, adds the
predecessor cleanup row and clears pending. It does not rewrite every evidence row.

Then open/verify the matching read snapshot and install PublishedView under a short manager lock. Another
publication waits until installation or recovery completes. No object/backend I/O occurs inside a redb writer.
[Publication](../search-index-qdrant/search-publication/FUNCTIONS.md) owns effect uncertainty and recovery.

## Query lookup

Resolve returned source_id/point_id in the pinned evidence table; validate identity/profile and read the exact
object/range. Missing, foreign or conflicting evidence fails closed. Resolve source scope against the same
published_sources snapshot.

Base source/access population controls candidates and IDF; request-only narrowing is candidate-only. Entire-base
selection omits redundant base/IDF clauses but preserves request-only filters. Stage 2 needs no filter indexes;
Stage 3A creates only fields used by real scope clauses. Scope semantics live with the query owner.

## Conditional Stage 3B and cleanup

Only failed Stage 3A budgets justify bounded source-prefix old/new merging and complete-source pending deltas.
The publisher owns fixed births/closures and exact effect/scoring convergence. Do not add another history store or
manifest cache. Oversize delta uses a fresh generation or explicit gap.

RetiredCleanup names a whole collection, not point pages/cursors. Remove cleanup/obsolete metadata only after the
publisher establishes eligibility and exact logical absence with no targetable unresolved attempt. Local immutable
objects remain stopped-root maintenance's responsibility.

## Removal and verification

Remove generic operation journals, copied ControlSnapshot, input replay, receipt scans, obsolete migration/grant/
key state and supplied in-memory floors with consumers. Keep direct typed transactions, exact reads and snapshots.

After code run minimal locked check/Clippy for real consumers. Focused cases: interrupted/empty capture, fixed-input
restart, bounded staging/final commit, minimal routing payload, physical/query separation, old-view binding,
commit/install gap, concurrent desired-state change, whole-generation cleanup and retained-page growth.
