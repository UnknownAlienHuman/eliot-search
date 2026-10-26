# Control state and evidence lookup

Use redb for compact current/published metadata and real read snapshots. Immutable objects hold source/prepared
bodies; Qdrant holds rebuildable retrieval projections. Remove generic journals, copied snapshots, receipts and
in-memory publication floors rather than rebuilding them under new names.

## Required typed state

Exact table/type names may differ, but responsibilities are fixed:

```text
root/settings/current source rows
current preparation outcomes
PendingGenerationBuild
published_sources[g]
published_evidence[g]
CurrentPublishedGeneration
RetiredCleanup
```

For one physical generation:

```text
published_sources[g]: source_id
  -> exact retained/prepared refs + selected outcome/gap

published_evidence[g]: (source_id, point_id)
  -> complete point/retrieval identity + ObjectRef/representation/range
```

These are compact metadata, not bodies, retrieval text, vectors or another search index. Exact key lookup resolves
a nomination; source-prefix range supports Stage 3B only if activated. Qdrant payload is routing data and never
replaces the pinned evidence row.

## `PublishedView`

Stage 2/3A state is:

```text
PublishedViewState {
  physical_collection
  retrieval_profile
  owned redb ReadTransaction
}
```

There is no mandatory sequence, route epoch or arbitrary historical-view lookup. Query clones current under a short
manager lock and opens tables through that same read transaction. It never opens a fresh latest read for a later
page of an old view.

Long read transactions retain pages. Bound request/window lifetime and publication backlog; measure retained-page
and old-collection disk, not only Arc counts. Last guard drop only wakes cleanup.

If Stage 3B is activated, add exact generation/profile/sequence fields needed by its validity model. Do not store
those fields in the baseline merely because an issue exists.

## Complete generation capture and build

A process-held read transaction is not a durable build manifest. Capture one complete desired input before Qdrant
effects:

1. open one consistent read over selected source/preparation outcomes and profile;
2. copy compact exact refs/outcomes into target-generation source rows in bounded transactions;
3. mark input capture complete only after enumeration and validation finish;
4. before completion, interruption discards the metadata-only target and restarts capture;
5. after completion, all generation pages/evidence are derived from those fixed refs;
6. missing/corrupt input fails the generation rather than substituting newer live state.

Stage 2/3A generation pages contain actual bounded retrieval text and payload data required by Qdrant 1.20. A
digest-only page cannot be dispatched. Point IDs are deterministic and generation-bound; no old/new occurrence
matching or validity interval is required.

After complete Qdrant schema/point/count/readiness verification:

```text
one bounded transaction:
  compare expected pending/current guards
  finalize complete published source/evidence tables
  set CurrentPublishedGeneration
  insert predecessor RetiredCleanup
  clear pending
commit
→ open/verify matching read snapshot
→ install PublishedView under short manager lock
```

No object or RPC I/O occurs inside the writer. Do not begin another publication until install succeeds or restart
recovery installs/refuses the durable current state.

## Query lookup and scope

`ResolvedScope` selects source IDs from the pinned `published_sources[g]`. Its base source/access selection becomes
both candidate eligibility and `idf.corpus`; request-only path/kind/topical narrowing affects candidates only.
Evidence validation point-reads `(source_id, point_id)` in the pinned table and fails closed on missing/foreign or
identity mismatch.

Stage 2's one-file collection equals its complete base population, so query omits redundant filter/IDF. Explicit
empty and unknown/incomplete remain distinct.

## Stage 3B conditional delta

Only if predeclared Stage 3A budgets fail, add one complete-source pending delta and source-prefix merge:

```text
fixed old/new descriptors + operation-fixed births
→ insert_only new points
→ fixed non-creating closures
→ exact readback
→ one bounded complete-source metadata/current commit
```

Do not create a second publisher, permanent operation history or whole-manifest cache. Oversize source delta falls
back to a fresh generation or explicit gap.

## Cleanup and other current state

`RetiredCleanup` names an exact whole physical collection. Publication/bridge owns terminal delete and exact
logical-absence observation. No point-ID cleanup pages, watermarks or reclaimer journal in Stage 2/3A.

Current source/preparation may be ahead of published state. Query reports the pinned published revision and known
pending/gap status; it never repairs ingestion. Uncertain local commits resolve by reading relevant typed rows.
Successful reads and pure transforms create no completed-operation archive.

## Removal and verification

Remove generic operation journal, copied ControlSnapshot, original-input replay, receipt scans, obsolete
migration/grant/key state and supplied in-memory floors with callers. Keep typed transactions, point/range reads and
real snapshot ownership.

After implementation verify complete/empty capture, interrupted capture before effects, fixed-input restart after
effects, bounded pages/transactions, exact evidence lookup, old-view binding, current commit/install gap,
source/profile change during build, whole-generation cleanup rows and retained-page growth. Stage 3B source-prefix
cases run only if that profile is selected. R051 changes instructions only.
