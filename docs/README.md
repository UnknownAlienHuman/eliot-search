# ELIOT Search documentation

This tree is organized for progressive reading. Start with the product contract, descend to the affected owner and
use audit records only when evidence or history is needed.

## Authority

| Layer | Owns | Does not own |
|---|---|---|
| [Architecture](architecture/README.md) | Product purpose, boundary and observable requirements | Package topology or current execution status |
| [Implementation](implementation/README.md) | Selected mechanisms, delivery sequence, lock order and optimization gates | Revision-specific evidence |
| Nearest `FUNCTIONS.md` | Exact owner seam, current defects and operation recipe | Product-wide mission or second roadmap |
| [Product behavior](product/README.md) | User-visible Search, Exact, orientation, comparison and document outcomes | Internal package status |
| [Analysis registry](audit/ANALYSIS_REGISTRY.md) | Current checkpoint, inspected facts and unrun work | Architecture or runtime permission |
| [Donor register](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) | Installed/candidate versions and verified upstream limits | Automatic adoption decisions |

When documents conflict, correct the owning layer and make other pages link to it. Do not satisfy both by adding a
compatibility path, receipt or second registry.

## Reading path by task

| Task | Read |
|---|---|
| Understand the product | [Architecture](architecture/README.md) → [product workflows](product/AGENT_ANALYSIS_FRAMEWORK.md) |
| Implement a slice | [Delivery sequence](implementation/README.md#delivery-sequence) → nearest owner recipe → actual callers → owning issue |
| Resume interrupted work | [R049 resume](audit/ANALYSIS_REGISTRY.md#resume) → affected owner only |
| Change a dependency/service | [Donor register](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) → consuming owner recipe |
| Move/delete legacy code/tests | [Test cutover](implementation/TEST_CUTOVER.md) and current caller search |
| Inspect data/effect/lock ownership | [Dependency graph](handoff/DEPENDENCY_GRAPH.md) |

## Current implementation decisions

R049 selects:

- owner-private handle-based Windows root/object finalization; `tempfile::persist_noclobber` is not the trusted
  Windows finalizer;
- Qdrant Server/Rust client 1.20.x for the implementation cut;
- direct BM25 over a scoring full-text payload index with explicit `code` tokenizer and `TextQuery`;
- exact physical collection identity in `PublishedView`, without aliases;
- Stage 3A complete immutable generations as the correctness/reference profile;
- numeric release budgets before Stage 3A may ship for a corpus class;
- Stage 3B point-level incrementality only where those budgets fail;
- no generic custom-shard partitioning because BM25 statistics are shard-local;
- Qdrant Edge as an isolated later candidate, not the current baseline.

The product remains unimplemented. The Cargo lock still contains Qdrant client 1.19.0; R049 changes documentation
and issue metadata only.

## Focused supporting material

- [Configuration](config/README.md)
- [Qdrant adapter boundary](runtime/QDRANT_ADAPTER_BOUNDARY.md)
- [Windows Qdrant notes](runtime/QDRANT_NATIVE_WINDOWS.md)
- [ADRs](adr/README.md)
- [Audit navigation](audit/README.md)
- [Handoff/navigation](handoff/README.md)

Retired Architecture 8.4, waves, packets, receipts, generated maps, standalone applications and structural
qualification remain in Git history. They may explain findings but cannot authorize work or restore dead APIs.
