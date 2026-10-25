# ELIOT Search documentation

This tree is organized for progressive reading. Start with the product contract, descend to the affected owner,
and use audit records only when evidence or history is needed.

## Authority

| Layer | Owns | Does not own |
|---|---|---|
| [Architecture](architecture/README.md) | Purpose, product boundary and observable requirements | Package topology or current implementation status |
| [Implementation](implementation/README.md) | Selected mechanisms, owners, delivery sequence and removal rules | Revision-specific execution evidence |
| Nearest `FUNCTIONS.md` | Exact source seams, inspected defects and operation recipe | Product-wide mission or permanent caller truth |
| [Product behavior](product/README.md) | User-visible Search, Exact, orientation, comparison and document outcomes | Internal package status |
| [Analysis registry](audit/ANALYSIS_REGISTRY.md) | Current checkpoint, inspected evidence and unrun work | Permission, architecture or readiness credentials |
| [Donor register](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) | Installed/candidate versions and verified upstream limits | Automatic implementation decisions or permanent future pins |

When documents conflict, correct the owning layer and make the others link to it. Do not satisfy both by adding
compatibility code or another registry.

## Reading path by task

| Task | Read |
|---|---|
| Understand the product | [Architecture](architecture/README.md) → [product workflows](product/AGENT_ANALYSIS_FRAMEWORK.md) |
| Implement a slice | [Delivery sequence](implementation/README.md#delivery-sequence) → affected owner in [Owners](implementation/README.md#owners) → nearest `FUNCTIONS.md` and exact current callers |
| Resume interrupted work | [Registry resume](audit/ANALYSIS_REGISTRY.md#resume-after-interruption) → affected owner and immutable evidence link |
| Change a dependency/service | [Donor register](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) → consuming owner recipe |
| Move/delete legacy code or tests | [Test cutover](implementation/TEST_CUTOVER.md) and [baseline member disposition](audit/WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md) |
| Inspect effect/data ownership | [Dependency graph](handoff/DEPENDENCY_GRAPH.md) |

## Current status

R044 completes the active implementation-document semantic pass. Query now has one owner for typed dispatch,
pinned scope, retrieval, verified evidence and one `QueryWindow`; planner, comparator, subject-resolver and handle
package audits no longer present historical sizes/test-only callers as permanent current facts. Projection and
client/CLI cutovers use the same rule. Before deleting or merging code, the implementation commit rechecks exact
manifests, Rust/non-Rust callers, constructors/module inclusion, features, targets, tests and workflows.

R043 completed runtime ownership: one real root guard, concrete secret owners, #598-controlled encrypted-object
handoff and no speculative old Windows transport. R042 corrected source/preparation/enrichment/supervisor/overlay
currentness and separated stopped-root maintenance from publication-owned Qdrant cleanup. R041–R037 reconciled
adapter/model-worker, document, MCP, regex and watcher handoffs. R031 adopted `pulldown-cmark 0.13.5` and passed
Rust 1.98 check/minimal Clippy for materializer + unitizer only.

The canonical active documentation is now internally aligned for implementation. Historical audit snapshots and
retired documents remain immutable evidence in Git; they are not rewritten into current authority. This does not
mean the product is implemented or runtime-verified.

The persistent source → Qdrant → retained-evidence → CLI path remains unconnected. The next implementation
slice is [typed settings and real consumers](../crates/search-config/FUNCTIONS.md), coordinated by
[#333](https://github.com/UnknownAlienHuman/eliot-search/issues/333), then source/object/preparation/reconciliation
and the connected CLI path. Current evidence and remaining delivery groups are in the
[registry](audit/ANALYSIS_REGISTRY.md).

## Focused supporting material

- [Configuration](config/README.md)
- [Qdrant adapter boundary](runtime/QDRANT_ADAPTER_BOUNDARY.md)
- [Windows Qdrant notes](runtime/QDRANT_NATIVE_WINDOWS.md)
- [ADRs](adr/README.md)
- [Audit navigation](audit/README.md)
- [Handoff/navigation](handoff/README.md)

Retired Architecture 8.4, waves, packets, receipts, generated maps, standalone applications and structural
qualification remain in Git history. They may explain a finding but cannot authorize work or restore dead APIs.
