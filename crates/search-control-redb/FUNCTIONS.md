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

## Plain-root control marker

Fresh plain roots use redb 4.5 in the same `control/control.redb` file. The only format marker is
`plain_control_meta["format_version"] = 1`. Legacy redb 2.6 state remains under its existing owner; this
path does not upgrade it or open an existing non-empty database for writing.

Initialization runs under root exclusion after the exact init marker is present. A missing file is created
with create-new semantics. An existing zero-length file is reopened without truncation and its opened handle
is checked again before initialization. Existing non-zero files use redb's read-only backend; an interrupted
initialization retry succeeds only when the bounded semantic-empty probe finds the exact current format row,
one regular table, no multimap tables, and no additional metadata rows. Initialization commits the marker,
explicitly closes the writer, then reads the format marker back through a read-only handle before the root
manifest can be published.

Ordinary current-root inspection reads only the fixed metadata table/key and version. It does not enumerate the
catalogue and has no whole-file size ceiling, so populated current databases remain inspectable. The separate
`plain_control_is_empty` probe is only for unfinished initialization or discard under root exclusion. It checks
the actual opened file against a 64 MiB ceiling before catalogue enumeration; larger files return `ProbeLimit`.
The fresh database's native size has not been measured in this source slice. Read-only open refuses databases
that redb says require repair; it never repairs them.
