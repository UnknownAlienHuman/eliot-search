# Indexing and publication

The [implementation spine](../../docs/implementation/README.md) defines one coherent publication/query path. This
directory groups existing code; the package list is not the target architecture.

| Existing code | Current direction |
|---|---|
| [projection planner](search-projection-planner/) | Stage 2/3A assembles complete generation pages. Changed-source diff/retain/close planning is Stage 3B only where measured budgets require it. |
| [point identity](search-point-identity/) | Stage 2/3A uses deterministic generation-bound IDs plus full collision evidence. Stable occurrence identity is conditional Stage 3B work. |
| [Qdrant supervisor](search-qdrant-supervisor/) | Optional managed-server convenience only. Baseline connects to an external Qdrant Server 1.20 endpoint. Recheck current callers before replacing/deleting. |
| [Qdrant bridge](search-qdrant-bridge/) | One private Qdrant 1.20 client/runtime for scoring full-text schema, insert-only build, `TextQuery`, scope/IDF, readback and whole-collection deletion. |
| [publication](search-publication/) | One fixed pending generation input, complete backend build, atomic evidence/current commit, explicit view install and finite cleanup rows. |
| [epoch pins](search-epoch-pins/) | Extract one small current-view manager; pin exact physical collection + redb snapshot. Delete the package after callers move. |
| [index reclaimer](search-index-reclaimer/) | Move only whole-collection delete/absence helper into publication/bridge, then delete the old point-reclaimer platform. |

## Baseline profile

The implementation cut upgrades the current locked `qdrant-client 1.19.0` and server profile together to 1.20.x:

```text
retrieval text payload
→ scoring full-text index with explicit code tokenizer/text settings
→ insert_only complete generation build
→ complete evidence/current commit
→ PublishedView with exact physical collection
→ TextQuery
```

No baseline sparse BM25 vector inference, fixed local `avg_len`, serving alias, custom-shard portfolio or embedded
Edge fallback.

Stage 3A complete generations are the correctness/reference implementation. They ship for a corpus class only if
#215's predeclared build/edit latency, throughput and disk budgets pass. Stage 3B point-level mutation is required
where they fail; otherwise it remains absent.

## Effect and lifetime boundary

One publisher owns external effects. A timeout/reconnect/client drop never becomes rollback proof. A failed or
uncertain generation is never served and its name is never reused. Only a complete durable evidence/current commit
followed by explicit `PublishedView` installation makes a generation visible.

Queries pin current under a short manager lock and then release it before Qdrant/object work. Old physical
collections and redb pages remain while guards exist. Last guard drop only wakes cleanup. Cleanup terminally
deletes an exact retired collection and verifies logical absence outside redb/view locks.

Qdrant Edge may be evaluated later as an isolated replacement candidate after the server path works. It never
creates a permanent dual backend.
