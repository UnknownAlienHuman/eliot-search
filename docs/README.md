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
| Implement a slice | [Delivery sequence](implementation/README.md#delivery-sequence) → nearest owner recipe → current callers → owning issue |
| Resume interrupted work | [R050 resume](audit/ANALYSIS_REGISTRY.md#resume) → affected owner only |
| Change dependency/service | [Donor register](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) → consuming owner recipe |
| Move/delete legacy code/tests | [Test cutover](implementation/TEST_CUTOVER.md) and current caller search |
| Inspect data/effect/lock ownership | [Dependency graph](handoff/DEPENDENCY_GRAPH.md) |

## Current implementation decisions

R049 selected Qdrant 1.20 direct full-text BM25, handle-safe Windows finalization, measured Stage 3A release gates,
conditional Stage 3B, no generic custom-shard portfolio and Edge as a later isolated candidate.

R050 synchronizes the remaining active Qdrant package/runtime instructions:

- Stage 2/3A projection assembles complete generations with actual retrieval-text payloads;
- Stage 2/3A point IDs are deterministic and generation-bound;
- stable occurrence identity, source-local retain/create/close and sequence ranges are Stage 3B only;
- baseline cleanup deletes whole physical collections only;
- the old point-reclaimer/possible-writer/checkpoint platform is removed after its small helper moves;
- view lifetime pins physical collection + redb snapshot, with sequence only in Stage 3B;
- historical Qdrant 1.19 Windows smoke is not current Qdrant 1.20 evidence.

The product remains unimplemented. Cargo still locks Qdrant client 1.19.0; R050 changes documentation and metadata
only.

## Focused supporting material

- [Configuration](config/README.md)
- [Qdrant adapter boundary](runtime/QDRANT_ADAPTER_BOUNDARY.md)
- [Windows Qdrant notes](runtime/QDRANT_NATIVE_WINDOWS.md)
- [ADRs](adr/README.md)
- [Audit navigation](audit/README.md)
- [Handoff/navigation](handoff/README.md)

Retired Architecture 8.4, waves, packets, receipts, generated maps, standalone applications and structural
qualification remain in Git history. They may explain findings but cannot authorize work or restore dead APIs.
