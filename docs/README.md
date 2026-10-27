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
| Resume interrupted work | [R054 resume](audit/ANALYSIS_REGISTRY.md#resume) → affected owner only |
| Change donor/service | [Donor register](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) → consuming owner |
| Move/delete legacy tests/code | [Test cutover](implementation/TEST_CUTOVER.md) → exact current caller search |
| Inspect ownership graph | [Dependency graph](handoff/DEPENDENCY_GRAPH.md) |

## Current status — R054

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

R053 correctly found that the reachable legacy source-registration Win32 wrapper is not a trusted relative-parent
finalizer. It then overreached by selecting a private direct NT call as the baseline. R054 removes that prescription.

Selected Windows boundary:

```text
retain verified handles for every directory from data root to final parent
  without FILE_SHARE_DELETE
retain opened stage handle with DELETE access
derive canonical absolute destination from the retained final-parent handle
→ documented SetFileInformationByHandle(FileRenameInfo)
   RootDirectory=NULL; ReplaceIfExists=FALSE
→ reopen final relative to retained parent
→ exact identity/bytes verification
```

Official Win32 documentation describes relative `RootDirectory`, but a current supported-build report shows the
wrapper may absolutize the name and fail when the root handle remains set. The implementation therefore does not
use that form. It also does not call private user-mode `NtSetInformationFile`. The retained handle chain prevents
rename/delete replacement of path components while the documented absolute-target operation runs.

The operation remains a canary until executed on each advertised Windows/local-filesystem profile. Unsupported
profiles refuse; there is no `MoveFileExW`, relative-root-wrapper, private-NT or generic-filesystem fallback. The
legacy source-root publication route is removed with callers rather than repaired as a compatibility platform.

Physical schema/resource changes create a fresh Qdrant generation. Query-time k/b changes do not. Stage 2 creates
no filter indexes. Query requests no vectors and only evidence-routing payload; excerpts come from pinned immutable
objects.

The repository still locks `qdrant-client 1.19.0`. No Rust/runtime change is claimed. Foundation, Windows
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
