# Control state and evidence lookup

Use redb for compact current/published metadata and real read snapshots. Immutable objects hold source/prepared
bodies; Qdrant holds rebuildable retrieval projections. Remove generic journals, copied snapshots, receipts and
in-memory publication floors rather than recreating them.

## Required typed state

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
  -> retained/prepared refs + selected outcome/gap

published_evidence[g]: (source_id, point_id)
  -> complete generation-bound point identity + ObjectRef/representation/range
```

These are compact metadata, not bodies, retrieval text, vectors or another search index. Exact key lookup resolves a
nomination. Source-prefix range exists only for Stage 3B if activated. Qdrant payload routes to the pinned row and
never replaces it.

## Physical view versus query scoring

Stage 2/3A durable/current view state contains only physical facts:

```text
PublishedViewState {
  physical_collection
  PhysicalTextIndexProfileId
  owned redb ReadTransaction
}
```

The physical profile identifies field/filter schema, tokenizer/normalization/scoring-index settings, text
composition, shard layout and explicit index residency. It does **not** contain query-time BM25 `k`/`b`.

Each admitted request separately captures:

```text
QueryScoringProfileV1 { k=1.2, b=0.75 }
```

Changing only k/b does not create a generation or mutate durable current state. A live `QueryWindow` stores final
ordered references and the original scoring profile for diagnostics; Continue never reruns the corpus.

Stage 2/3A has no mandatory sequence, route epoch or historical-view lookup. Query clones current under a short lock
and opens tables through that same read transaction. It never opens fresh latest metadata for a page of an old view.

Long read transactions retain pages. Bound request/window lifetime and publication backlog; measure retained redb
pages plus old physical collections, not Arc counts. Last guard drop only wakes cleanup.

## Complete generation capture

A process-held read transaction is not a durable build manifest. Capture one complete desired input before Qdrant
effects:

1. open one consistent read over selected source/preparation outcomes and physical profile;
2. copy compact exact refs/outcomes into target-generation source rows in bounded transactions;
3. mark capture complete only after enumeration/validation finish;
4. before completion, interruption discards the metadata-only target and restarts capture;
5. after completion, all pages/evidence derive from those fixed refs;
6. missing/corrupt input fails rather than substituting newer live state.

Stage 2/3A pages contain actual bounded `retrieval_text` and routing/filter payload required by Qdrant 1.20. A
digest-only page cannot be dispatched. Point IDs are deterministic and generation-bound; no old/new occurrence
matching or validity interval.

After complete backend readiness—including exact schema/point/count readback and deterministic TextQuery canary:

```text
bounded transaction:
  compare expected pending/current guards
  finalize published source/evidence tables
  set CurrentPublishedGeneration
  add predecessor RetiredCleanup
  clear pending
commit
→ open/verify matching read snapshot
→ install PublishedView under short manager lock
```

No object/RPC I/O occurs inside the writer. Do not begin another publication until installation succeeds or restart
recovery installs/refuses the durable current state.

## Query lookup and response routing

Qdrant query returns point ID plus only required routing payload, normally `source_id`; it returns no vectors and no
`retrieval_text`. Resolve `(source_id, point_id)` in the pinned evidence table, validate physical profile/identity,
then open the exact immutable object/range. Missing/foreign mismatch fails closed.

`ResolvedScope` selects source IDs from pinned `published_sources[g]`:

```text
base population = selected sources ∩ actual access/session restriction
candidate filter = base population + request-only narrowing
idf.corpus       = base population
```

Stage 2 one-file selection omits filters/IDF and requires no filter index. Stage 3A creates and records only indexes
used by actual scope fields.

## Stage 3B conditional delta

Only if predeclared Stage 3A budgets fail, add one complete-source pending delta and source-prefix merge:

```text
fixed old/new descriptors + operation-fixed births
→ InsertOnly new points
→ fixed non-creating closures
→ exact mutation and scoring/optimizer convergence checks
→ one bounded complete-source metadata/current commit
```

Do not add a second publisher, permanent operation history or whole-manifest cache. Oversize source delta falls back
to a fresh generation or explicit gap.

## Cleanup/removal/verification

`RetiredCleanup` names one exact whole physical collection. Publication/bridge owns terminal delete and exact logical
absence. No point-ID cleanup pages, watermarks or reclaimer journal in Stage 2/3A.

Remove generic operation journal, copied ControlSnapshot, input replay, receipt scans, obsolete migration/grant/key
state and supplied in-memory floors with callers. Keep typed transactions, exact point/range reads and snapshot
ownership.

After implementation verify complete/empty capture, interrupted capture before effects, fixed-input restart,
bounded pages/transactions, exact evidence routing with minimal Qdrant payload, physical/query-profile separation,
old-view binding, commit/install gap, source/profile change during build, whole-generation cleanup rows and retained
page growth. Stage 3B source-prefix cases run only if selected. R052 changes docs only.
