# ELIOT Search documentation

The active documentation is organized for implementation, not archaeology. Start with the product contract,
follow the vertical delivery spine, then read only the owner and current callers touched by the cut.

## Authority

| Layer | Owns | Does not own |
|---|---|---|
| [Architecture](architecture/README.md) | Product boundary and observable requirements | Package topology or current status |
| [Implementation spine](implementation/README.md) | Delivery order, cross-owner invariants and lock/effect boundaries | Revision-specific evidence |
| Nearest `FUNCTIONS.md` | Exact local seams, failure rules and current replacement | Product-wide mission or permanent caller truth |
| [Product behavior](product/README.md) | User-visible Search, Exact, orientation, comparison and documents | Internal package status |
| [Analysis registry](audit/ANALYSIS_REGISTRY.md) | Current checkpoint, findings, published decisions and unrun work | Architecture, permission or prerequisite authority |
| [Donor register](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) | Installed/candidate versions and verified upstream limits | Automatic adoption or future pin |
| Open GitHub issue | Work owner and completion evidence | Independent architecture or prerequisite graph |

When an issue or secondary README conflicts with the owning layer, correct it before implementation. Do not
support both versions through compatibility code.

## Reading path

| Task | Read |
|---|---|
| Understand the product | [Architecture](architecture/README.md) |
| Start implementation | [Delivery sequence](implementation/README.md#delivery-sequence) and [lock discipline](implementation/README.md#lifetime-and-lock-order) |
| Implement one cut | Nearest owner recipe, actual current callers and the owning issue |
| Resume after interruption | [Registry resume](audit/ANALYSIS_REGISTRY.md#resume) |
| Change a dependency/service | [Donor register](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) and its real consumer |
| Move/delete legacy code or tests | [Test cutover](implementation/TEST_CUTOVER.md) and exact current caller search |
| Inspect data/effect flow | [Dependency graph](handoff/DEPENDENCY_GRAPH.md) |

## R047 verification result

R047 is a hostile-read verification of R046, not another architecture pass. It closes the remaining active
contradictions which could make an agent re-enter the old bureaucracy:

- root README and QUICKSTART now point to Foundation then the one-file walking skeleton;
- #598 is optional encrypted storage, not a prerequisite for the plain baseline;
- root/source/Qdrant/release/view issues are synchronized with the current spine;
- typed settings remain incremental and consumer-driven;
- Qdrant publication explicitly uses `insert_only`, a non-creating fixed closure and no per-point deletion;
- whole-generation cleanup is outside the critical path and remains durable/retryable until exact logical absence
  is observed; collection names are never reused and physical disk erasure is not claimed;
- the donor checkpoint and active anchors are current;
- historical issue bodies, audit snapshots and closed waves remain immutable evidence only.

This establishes a coherent implementation plan, not a working product. Current binaries, Windows durability,
live Qdrant, restart/recovery, relevance, scale and end-to-end behavior remain unrun.

## Current implementation order

```text
Foundation
→ one explicit file / plain ObjectRef
→ preparation
→ one fresh Qdrant generation
→ one Search
→ pinned CLI excerpt
→ convergence/restart
→ Exact/live session
→ maintenance and selected optional depth
```

The baseline does not wait for encrypted storage, watcher, regex, documents, MCP, models, overlay, managed
Qdrant, persistent pipe or broad qualification.

## Historical material

Retired Architecture 8.4, waves, packets, receipts, generated maps and immutable audit records remain in Git
history. They may explain why code exists but cannot authorize new work, restore removed compatibility or block a
vertical cut.
