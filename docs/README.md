# Search documentation

Keep the implementation context small.

## Current authority

| Document | Purpose |
|---|---|
| [Architecture](architecture/README.md) | Product purpose, observable requirements, boundaries and principles. |
| [Implementation](implementation/README.md) | Current mechanisms, owners, cutover order, removals and selected donors. |
| [Product behavior](product/AGENT_ANALYSIS_FRAMEWORK.md) | Useful navigation, ambiguity, comparison, document and exact-verification outcomes. |
| [Static audit closure](audit/AUDIT_COVERAGE_AND_DOCUMENTATION_MAP_2026-10-21.md) | Checked surface, replacement mistakes and remaining runtime evidence. |
| [Current ownership graph](handoff/DEPENDENCY_GRAPH.md) | Small effect/data ownership graph and cutover order. |

The nearest package or binary `FUNCTIONS.md` owns one concrete operation recipe and source seams. The active issue names the exact callers, deletion and completion evidence for that change.

## Supporting current material

- [Configuration](config/) records the typed-settings replacement and direct runtime-change semantics.
- [Qdrant runtime boundary](runtime/QDRANT_ADAPTER_BOUNDARY.md) and [Windows/runtime notes](runtime/QDRANT_NATIVE_WINDOWS.md) describe the selected indexed backend seam.
- [ADRs](adr/) retain only decisions that still constrain the current product or packaging.
- [Audit](audit/) and donor records explain evidence and limitations; they do not authorize code or create gates.

## Historical material

Architecture 8.4 masters, P00/W0–W10 contracts, dated execution packets, client/current/evaluation/optional wave bundles, generated maps, standalone DIRECT/DPAPI runbooks, handoff receipts, swarm assignments and structural qualification records were removed from the current tree. Git history is their archive.

Do not regenerate those records, preserve their APIs through adapters or use an old title containing `CURRENT`, `CONTRACT`, `GATE`, `ACCEPTANCE`, `STATUS` or `IMPLEMENTATION` as current authority. A useful historical fact must be rechecked against current code and placed in its real owner document.
