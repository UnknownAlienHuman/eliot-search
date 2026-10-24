# Architecture decision records

ADRs preserve decisions whose rationale remains useful. They do not form a second architecture or a required
package/status graph. Current purpose lives in [Architecture](../architecture/README.md); current mechanisms and
owners live in [Implementation](../implementation/README.md).

| ADR | Current status | Meaning |
|---|---|---|
| [0001 — Capability boundaries and Cargo packaging](0001-capability-cell-crate-decomposition.md) | Accepted packaging guidance | Split only for real state/effect/dependency/replacement boundaries; no crate-count or line-count target. |
| [0005 — Standalone Search and controller boundary](0005-standalone-search-product-and-controller-boundary.md) | Accepted | Search works through its own CLI/daemon; ELIOT/controller integration is an optional leaf. |
| [0006 — Agent-oriented analysis and retrieval](0006-agent-analysis-framework-product-scope.md) | Product mission retained; old 8.4 prescriptions superseded | Orientation, evidence, comparison, documents and exact verification remain requirements through the current source/query path. |

Historical ADR text may explain why a mechanism existed. When it conflicts with current architecture,
implementation owners or observed code, the current documents govern and the ADR should link to the superseding
decision rather than force compatibility.
