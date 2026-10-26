# ELIOT Search documentation

Read progressively: product contract → implementation sequence → affected owner/callers → current evidence. Audit
records do not authorize code or create readiness credentials.

## Authority

| Layer | Owns |
|---|---|
| [Architecture](architecture/README.md) | Product purpose, boundary and observable requirements |
| [Implementation spine](implementation/README.md) | Selected mechanisms, delivery order, ownership and removal rules |
| Nearest `FUNCTIONS.md` | Exact source seams, algorithm and failure rules |
| [Product behavior](product/README.md) | User-visible Search/Exact/orientation/comparison/document outcomes |
| [Analysis registry](audit/ANALYSIS_REGISTRY.md) | Current checkpoint, inspected evidence and unrun work |
| [Donor register](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) | Exact upstream versions/APIs and applicability limits |

When text conflicts, correct the owning layer and make the other document link to it. Do not satisfy both by adding
compatibility code, a second route or another registry.

## Reading path

| Task | Read |
|---|---|
| Understand product | [Architecture](architecture/README.md) → [product workflows](product/AGENT_ANALYSIS_FRAMEWORK.md) |
| Implement a slice | [Delivery sequence](implementation/README.md#delivery-sequence) → nearest owner recipe → actual callers |
| Resume interrupted work | [R051 resume](audit/ANALYSIS_REGISTRY.md#resume) → affected owner only |
| Change donor/service | [Donor register](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) → consuming owner |
| Move/delete legacy tests/code | [Test cutover](implementation/TEST_CUTOVER.md) → exact current caller search |
| Inspect ownership graph | [Dependency graph](handoff/DEPENDENCY_GRAPH.md) |

## Current status — R051

R051 verifies the exact official Qdrant Rust client 1.20 text-search API and reconciles lexical, query, scope,
retrieval executor, unitizer, control and bridge owners.

Selected implementation:

```text
vectorless physical collection
+ retrieval_text payload
+ scoring Text index (Code tokenizer)
+ insert_only points
+ deterministic TextQuery readiness canary
+ TextQuery with explicit k/b
+ base-population idf.corpus when source/access selection narrows collection
```

Removed from the mandatory baseline:

- sparse `qdrant/bm25` document/query inference and fixed local `avg_len`;
- local postings/numerical BM25;
- automatic snake/camel/acronym alias expansion;
- hidden Boolean/phrase parsing of free query text;
- old/new occurrence matching, stable point identity and sequence in Stage 2/3A;
- query scheduler/tickets/leg fusion and point-level cleanup platform.

The Cargo lock still contains client 1.19.0. No Rust/runtime change is claimed. Foundation, Windows handle
finalization, Qdrant 1.20, walking skeleton, restart, relevance and scale remain unrun.

## Focused supporting material

- [Configuration](config/README.md)
- [Qdrant adapter boundary](runtime/QDRANT_ADAPTER_BOUNDARY.md)
- [Windows Qdrant notes](runtime/QDRANT_NATIVE_WINDOWS.md)
- [ADRs](adr/README.md)
- [Audit navigation](audit/README.md)
- [Handoff](handoff/README.md)

Historical Architecture 8.4, waves, packets, receipts, generated maps and standalone applications remain in Git
history. They may explain a finding but cannot restore dead APIs or block the current vertical path.
