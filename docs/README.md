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

R042 extends the revision-currentness rule to source, runtime, preparation, code-enricher, Qdrant-supervisor and
overlay package instructions. Historical byte counts and no-caller searches remain useful evidence only at their
inspected revisions. A deletion slice must recheck exact current manifests, Rust/non-Rust callers, feature/target
wiring, tests and workflows.

R042 also corrects maintenance ownership: the removed `search-retention` Cargo package is not restored;
documentation under that path owns stopped-root local-object collection and archive/import, while online Qdrant
retirement remains publication cleanup.

R041 aligned adapter/model-worker currentness and the donor checkpoint. R040 aligned the document worker with
PDFium as an uninstalled implementation-time candidate. R039 updated the inspected stable MCP SDK to `rmcp
3.7.0` while keeping it absent from the ELIOT lock. R038 keeps regex optional/uninstalled; R037 defers watcher
work until reconciliation functions independently. R031 adopted `pulldown-cmark 0.13.5` and passed Rust 1.98
check/minimal Clippy for materializer + unitizer only.

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
