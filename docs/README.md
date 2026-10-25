# ELIOT Search documentation

This tree is organized for progressive reading. Start with the product contract, descend to the affected owner,
and use audit records only when evidence or history is needed.

## Authority

| Layer | Owns | Does not own |
|---|---|---|
| [Architecture](architecture/README.md) | Purpose, product boundary and observable requirements | Package topology or current implementation status |
| [Implementation](implementation/README.md) | Selected mechanisms, owners, delivery sequence and removal rules | Revision-specific execution evidence |
| Nearest `FUNCTIONS.md` | Exact source seams, current defects and operation recipe | Product-wide mission or a second roadmap |
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

R043 completes the runtime-family documentation correction. Root ownership is one retained current-format root
and exclusive process-lifetime lock, not a lease/epoch/receipt protocol. Generic secret lifecycle is removed in
favor of concrete Qdrant, restricted-client or immutable-object owners. `search-revision-crypto` is retained only
if the accepted #598 handoff directly consumes it. The old client-only Windows named-pipe adapter is removed when
an exact current caller search confirms no persistent-daemon consumer.

Every historical size or caller audit is tied to its inspected revision. A code-deletion slice must repeat the
current search across manifests, Rust/non-Rust callers, constructors, feature/target wiring, tests and workflows,
then move all surviving callers in the same cut. Documentation does not turn an old audit into deletion authority.

R042 applied the same currentness rule to source, preparation, code-enricher, Qdrant-supervisor and overlay, and
separated stopped-root local-object maintenance/archive/import from publication-owned online Qdrant cleanup.
R041 corrected adapter/model-worker currentness and the donor checkpoint. R040–R037 reconcile document, MCP,
regex and watcher prerequisites. R031 adopted `pulldown-cmark 0.13.5` and passed Rust 1.98 check/minimal Clippy
for materializer + unitizer only.

The persistent source → Qdrant → retained-evidence → CLI path remains unconnected. The next implementation
slice is [typed settings and real consumers](../crates/search-config/FUNCTIONS.md), coordinated by
[#333](https://github.com/UnknownAlienHuman/eliot-search/issues/333). Current evidence and remaining delivery
groups are in the [registry](audit/ANALYSIS_REGISTRY.md).

## Focused supporting material

- [Configuration](config/README.md)
- [Qdrant adapter boundary](runtime/QDRANT_ADAPTER_BOUNDARY.md)
- [Windows Qdrant notes](runtime/QDRANT_NATIVE_WINDOWS.md)
- [ADRs](adr/README.md)
- [Audit navigation](audit/README.md)
- [Handoff/navigation](handoff/README.md)

Retired Architecture 8.4, waves, packets, receipts, generated maps, standalone applications and structural
qualification remain in Git history. They may explain a finding but cannot authorize work or restore dead APIs.
