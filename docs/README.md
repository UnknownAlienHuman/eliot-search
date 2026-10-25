# ELIOT Search documentation

Start with the product contract, then the implementation spine and only the owner recipe needed for the current
vertical cut.

## Authority

| Layer | Owns |
|---|---|
| [Architecture](architecture/README.md) | purpose, product boundary and observable behavior |
| [Implementation](implementation/README.md) | execution spine, owner boundaries and delivery order |
| Nearest `FUNCTIONS.md` | exact code seam, operation and failure rules |
| [Product behavior](product/README.md) | user-visible Search/Exact/orientation/comparison/document outcomes |
| [Analysis registry](audit/ANALYSIS_REGISTRY.md) | checkpoint, inspected evidence and unrun work |
| [Donor register](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) | versioned upstream facts and applicability limits |

Issues coordinate work; they do not override the owning document. Historical architecture, waves, packets,
receipts and generated maps remain evidence in Git only.

## Reading path

| Task | Read |
|---|---|
| Understand the product | [Architecture](architecture/README.md) |
| Implement the next cut | [Implementation spine](implementation/README.md#delivery-sequence) → affected owner recipe → actual callers |
| Resume interrupted work | [Registry](audit/ANALYSIS_REGISTRY.md#resume) |
| Change a dependency/service | [Donor register](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) → real consumer |
| Remove legacy code/tests | [Test cutover](implementation/TEST_CUTOVER.md) and current caller search |
| Inspect ownership flow | [Dependency graph](handoff/DEPENDENCY_GRAPH.md) |

## Current status — R046

The implementation-readiness audit found and corrected three blockers:

1. #333 was written as if settings for future source/Qdrant/query owners had to be completed first. Settings are
   now incremental; Slice 1 ships only root/plain-storage fields.
2. #330/#598 made encrypted storage a prerequisite for source retention. The plain current-format object profile
   is now the baseline; #598 is optional encrypted-profile work.
3. publication allowed individual point deletion, which forced an unresolved late-writer/isolated-generation
   recovery system. Baseline publication now keeps closed points, retries fixed insert-only creates/closures and
   deletes only whole retired never-reused generations.

The first implementation target is therefore a narrow vertical walking skeleton:

```text
root + plain object store
→ one explicit file
→ preparation
→ fresh Qdrant generation
→ one one-shot Search
→ pinned retained excerpt
→ CLI output
```

Reconciliation, live sessions, cleanup, encrypted storage and optional depth follow that proof. Documentation is
implementation-ready, but no current end-to-end runtime success is claimed.
