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
| [Donor register](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) | Exact upstream versions/APIs/defaults and applicability limits |

When text conflicts, correct the owning layer and make other documents link to it. Do not satisfy both by adding
compatibility code, a second route or another registry.

## Reading path

| Task | Read |
|---|---|
| Understand product | [Architecture](architecture/README.md) → [product workflows](product/AGENT_ANALYSIS_FRAMEWORK.md) |
| Implement a slice | [Delivery sequence](implementation/README.md#delivery-sequence) → nearest owner recipe → actual callers |
| Resume interrupted work | [R052 resume](audit/ANALYSIS_REGISTRY.md#resume) → affected owner only |
| Change donor/service | [Donor register](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) → consuming owner |
| Move/delete legacy tests/code | [Test cutover](implementation/TEST_CUTOVER.md) → exact current caller search |
| Inspect ownership graph | [Dependency graph](handoff/DEPENDENCY_GRAPH.md) |

## Current status — R052

R052 inspects Qdrant 1.20 payload-index defaults and exact tokenizer/filter/scoring behavior, then freezes a small
explicit baseline:

```text
PhysicalTextIndexProfileV1:
  vectorless one-shard collection
  retrieval_text Text index
  Code tokenizer, lowercase
  no ascii folding, phrase matching, stemmer, stopwords, token limits or nested path
  BM25 scoring
  payload HNSW disabled
  Stage-2 memory=Pinned

QueryScoringProfileV1:
  k=1.2
  b=0.75
  no score threshold
```

Physical schema/resource changes create a fresh generation. Query-time k/b changes do not. Stage 3A chooses
`Pinned|Cached|Cold` explicitly per measured corpus profile.

Stage 2 creates no filter indexes because one file is the complete selected population. Stage 3A creates only real
source/access/path-kind indexes, also with HNSW disabled and explicit memory.

The Code tokenizer supplies subwords and joined camel/snake identifiers to ranking queries; no local alias expansion.
Free text is any-token nomination. Full-text filters require all subwords in any order and are not exact identifiers
or phrases. Exact punctuation/location remains retained-source Exact.

Query asks Qdrant for no vectors and only evidence-routing payload. Excerpts always come from pinned immutable
objects. Readiness requires exact schema/resource readback and deterministic TextQuery canary.

The repository still locks client 1.19.0. No Rust/runtime change is claimed. Foundation, Windows finalization,
Qdrant 1.20, explicit text-index profile, walking skeleton, restart, relevance and scale remain unrun.

## Focused supporting material

- [Configuration](config/README.md)
- [Qdrant adapter boundary](runtime/QDRANT_ADAPTER_BOUNDARY.md)
- [Windows Qdrant notes](runtime/QDRANT_NATIVE_WINDOWS.md)
- [ADRs](adr/README.md)
- [Audit navigation](audit/README.md)
- [Handoff](handoff/README.md)

Historical Architecture 8.4, waves, packets, receipts, generated maps and standalone applications remain in Git
history. They may explain a finding but cannot restore dead APIs or block the current vertical path.
