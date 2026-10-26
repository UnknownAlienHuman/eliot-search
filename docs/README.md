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
| Resume interrupted work | [R053 resume](audit/ANALYSIS_REGISTRY.md#resume) → affected owner only |
| Change donor/service | [Donor register](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) → consuming owner |
| Move/delete legacy tests/code | [Test cutover](implementation/TEST_CUTOVER.md) → exact current caller search |
| Inspect ownership graph | [Dependency graph](handoff/DEPENDENCY_GRAPH.md) |

## Current status — R053

R052 froze the exact Qdrant 1.20 lexical baseline:

```text
PhysicalTextIndexProfileV1:
  vectorless one-shard collection
  retrieval_text Text index
  Code tokenizer, lowercase
  no ASCII folding, phrase matching, stemmer, stopwords, token limits or nested path
  BM25 scoring
  payload HNSW disabled
  Stage-2 memory=Pinned

QueryScoringProfileV1:
  k=1.2
  b=0.75
  no score threshold
```

R053 then traced the current Windows publication code and found a reachable counterexample: legacy
`--register-source-root`/`--unregister-source-root` reaches `registration_native::rename`, which passes a relative
leaf plus non-null parent handle through the Win32 `SetFileInformationByHandle(FileRenameInfo)` wrapper. That route
must not be reused as the root/object finalizer.

Selected Windows boundary:

```text
opened stage handle + retained destination-directory handle + relative final leaf
→ direct owner-private native rename information call, replacement disabled
→ exact status/ABI validation
→ reopen through retained parent
→ exact identity/bytes verification
```

The operation remains a canary until executed on the exact supported Windows/filesystem profile. Unsupported
profiles refuse; there is no path-only fallback and no generic filesystem framework. The legacy source-root route
is removed with its callers during the Foundation/source cut rather than strengthened as a compatibility platform.

Physical schema/resource changes create a fresh Qdrant generation. Query-time k/b changes do not. Stage 2 creates
no filter indexes. Query requests no vectors and only evidence-routing payload; excerpts come from pinned immutable
objects.

The repository still locks `qdrant-client 1.19.0`. No Rust/runtime change is claimed. Foundation, native Windows
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
