# ELIOT Search documentation

Start with product behavior, then the implementation spine and only the owner recipe needed for the current
vertical cut.

## Authority

| Layer | Owns |
|---|---|
| [Architecture](architecture/README.md) | purpose, product boundary and observable behavior |
| [Implementation](implementation/README.md) | execution spine, cross-owner invariants and delivery order |
| Nearest `FUNCTIONS.md` | exact code seam, operation and failure rules |
| [Product behavior](product/README.md) | user-visible Search/Exact/orientation/comparison/document outcomes |
| [Analysis registry](audit/ANALYSIS_REGISTRY.md) | checkpoint, inspected evidence and unrun work |
| [Donor register](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) | installed/candidate dependencies, upstream facts and analogue dispositions |

Issues coordinate ownership; they do not override the owning document. Historical waves, packets, receipts and
generated maps remain evidence in Git only.

## Reading path

| Task | Read |
|---|---|
| Understand the product | [Architecture](architecture/README.md) |
| Implement next cut | [Implementation spine](implementation/README.md#delivery-sequence) → affected owner recipe → actual callers |
| Resume work | [R048 registry](audit/ANALYSIS_REGISTRY.md#resume) |
| Change a dependency/service/backend | [Donor/analogue register](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) → real consumer |
| Remove legacy code/tests | [Test cutover](implementation/TEST_CUTOVER.md) plus exact current caller search |
| Inspect ownership/lock flow | [Dependency graph](handoff/DEPENDENCY_GRAPH.md) |

## Current status — R048

R048 inspected Git/tempfile, Tantivy, Zoekt, Watchman/ignore, qntx/qmd, Veles, codesearch and current Qdrant
mechanisms. The selected simplifications are:

1. plain object/root finalization prefers same-directory `tempfile::persist_noclobber` plus exact readback;
2. Foundation initialization is final-manifest-last with exact discard/retry, not resume/replay;
3. the first restart-convergent index rebuilds a complete fresh physical Qdrant generation for every real desired
   state change;
4. stable point reuse, fixed closures and compaction are a later measured optimization, not prerequisites;
5. `PublishedView` names a physical collection directly; Qdrant aliases are not serving authority;
6. filesystem notifications remain dirty hints only; authoritative inventory determines selected state;
7. Qdrant remains the only selected backend—no fallback trait or dual index before measurement.

Current vertical sequence:

```text
Foundation
→ one-file Search walking skeleton
→ restart + fresh-generation convergence
→ measured optional incrementality
→ Exact/live session
→ maintenance and selected depth
```

The application is still unimplemented. R048 changes instructions only and adds no compiler/runtime evidence.

## Focused material

- [Configuration](config/README.md)
- [Qdrant boundary](runtime/QDRANT_ADAPTER_BOUNDARY.md)
- [Windows Qdrant notes](runtime/QDRANT_NATIVE_WINDOWS.md)
- [ADRs](adr/README.md)
- [Audit navigation](audit/README.md)
- [Handoff/navigation](handoff/README.md)

Retired Architecture 8.4, waves, packets, receipts, generated maps and structural qualification cannot restore
dead APIs or become implementation prerequisites.