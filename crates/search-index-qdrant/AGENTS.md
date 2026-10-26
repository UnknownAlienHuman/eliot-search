# Working on indexing and publication

Read the [architecture](../../docs/architecture/README.md),
[implementation spine](../../docs/implementation/README.md) and nearest owner recipe. Qdrant stores rebuildable
derived retrieval state; immutable objects and committed evidence remain with their actual owners.

## Selected baseline

Connect one configured Qdrant Server/Rust client 1.20.x through one bridge/runtime before adding process
management. The current Cargo lock still contains client 1.19.0; upgrade dependency, lock, schema and actual
publication/query consumers together.

Use direct BM25 over one scoring full-text payload index with explicit `code` tokenizer/text processing and
`TextQuery`. Do not preserve baseline sparse BM25 vectors, query inference or fixed local `avg_len` merely because
old code exposes them.

`PublishedView` names an exact physical collection and matching redb snapshot. Do not add Qdrant alias authority.
Do not partition a general portfolio by custom shard key: BM25 statistics are shard-local and cross-shard scores
would require another fusion owner.

## Publication order

Use one publisher and one fixed pending generation input:

```text
complete fixed pages
→ complete insert_only physical build + exact readback
→ bounded evidence/inventory/current commit
→ open matching snapshot
→ short PublishedView install
```

No redb writer or view-manager mutex spans Qdrant/object I/O. A failed/uncertain generation is never served and its
name is never reused. Remove duplicate daemon/bridge journals, supplied commit proofs and completed-operation
history.

Stage 3A complete immutable generations are mandatory correctness/reference behavior. Measure build/edit latency,
writes, resources and retained-generation disk against #215's predeclared budgets. Add changed-source point deltas,
stable occurrence identity and fixed closures only where those budgets fail.

## Lifetime and cleanup

Query pins current under a short lock, then releases it before scope/backend/evidence work. A live window may clone
the same guard. Last guard drop only wakes cleanup.

Baseline cleanup deletes whole retired physical collections only after no current/pending/future target, live guard
or unresolved targetable attempt remains. Delete terminally, verify exact logical absence and never reuse the name.
Move only this useful helper from the old reclaimer package; delete point-delete manifests, writer ledgers,
watermarks, receipts and checkpoint machinery.

## Optional processes/backends

The current supervisor is disconnected and cannot apply claimed Windows containment. Managed Qdrant is an optional
small child/Job owner after the external-endpoint path works; otherwise delete it.

Qdrant Edge is an isolated later replacement candidate, not a fallback. Do not add a backend trait or dual writer.
A newer released Edge line must pass persistence/crash, Windows, full-text/BM25, filtering, generation and size/
latency evidence before replacing the bridge.

A planner, identity, bridge, publication or view-manager name does not require a permanent crate. Keep a boundary
only for a real dependency/effect or substantial algorithm. Change direct callers and delete displaced mechanisms
in the same vertical cut.
