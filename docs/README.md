# ELIOT Search documentation

This tree is organized for progressive reading. Start with the product contract, descend to the affected owner,
and use audit records only when evidence or history is needed.

## Authority

| Layer | Owns | Does not own |
|---|---|---|
| [Architecture](architecture/README.md) | Purpose, product boundary and observable requirements | Package topology, current implementation status |
| [Implementation](implementation/README.md) | Selected mechanisms, owners, delivery sequence and removal rules | Revision-specific execution evidence |
| Nearest `FUNCTIONS.md` | Exact source seams, current defects and operation recipe | Product-wide mission or a second roadmap |
| [Product behavior](product/README.md) | User-visible Search, Exact, orientation, comparison and document outcomes | Internal package status |
| [Analysis registry](audit/ANALYSIS_REGISTRY.md) | Current checkpoint, inspected evidence and unrun work | Permission, architecture or readiness credentials |
| [Donor register](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) | Installed/selected versions and verified upstream limits | Automatic implementation decisions |

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

R039 corrects the optional MCP donor state. `rmcp` is absent from the current lock; no MCP SDK/runtime is
installed. The stable specification remains `2026-07-28`; the latest stable official Rust SDK inspected is
`rmcp 3.7.0` (2026-10-22, commit `5d6a11f2…`, MSRV 1.88). Its auth/HTTP-only release changes do not justify
those features for the first stdio leaf. #219 must recheck and pin the then-current SDK only with a real
post-baseline consumer.

R038 made regex an implementation-time optional donor. R037 gated watcher work on functioning reconciliation.
R036 made PDFium an implementation-time candidate and removed repeated retention deletion. R035 reconciled
active issue evidence/currentness. R034 labelled historical package inventories. R033 fixed active package
entry claims. R032 established the authority chain and delivery sequence. R031 adopted `pulldown-cmark 0.13.5`
and passed Rust 1.98 check/minimal Clippy for materializer + unitizer.

The persistent source → Qdrant → retained-evidence → CLI path remains unconnected. The next implementation
slice is [typed settings and real consumers](../crates/search-config/FUNCTIONS.md), coordinated by
[#333](https://github.com/UnknownAlienHuman/eliot-search/issues/333). Current evidence and remaining groups are
in the [registry](audit/ANALYSIS_REGISTRY.md).

## Focused supporting material

- [Configuration](config/README.md)
- [Qdrant adapter boundary](runtime/QDRANT_ADAPTER_BOUNDARY.md)
- [Windows Qdrant notes](runtime/QDRANT_NATIVE_WINDOWS.md)
- [ADRs](adr/README.md)
- [Audit navigation](audit/README.md)
- [Handoff/navigation](handoff/README.md)

Retired Architecture 8.4, waves, packets, receipts, generated maps, standalone applications and structural
qualification remain in Git history. They may explain a finding but cannot authorize work or restore dead APIs.
