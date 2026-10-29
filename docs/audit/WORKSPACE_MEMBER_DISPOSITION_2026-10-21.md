# Workspace disposition — current rules

The original 52-row table was a baseline audit, not a package-count target. R030 removed `search-eval` on the unmerged
refactor branch; R045 counted 49 explicit members there. Main still has its older source/manifests. Before any deletion, recheck the current branch's manifests,
Rust/non-Rust callers, module inclusion, features, targets, tests and workflows.

Git history preserves the full historical table. This file records only current implementation rules so an agent
does not treat old counts or receiving owners as mandatory topology.

## Keep a boundary only when it isolates

- durable state or transaction ownership;
- an external/OS effect;
- a substantial reusable algorithm;
- a real optional adapter with a current consumer.

Otherwise move useful code to its consumer and delete the forwarding package and exclusive fixtures.

## Current family direction

| Family | Direction |
|---|---|
| Root/config/control | keep one root guard, small incremental typed settings and typed redb methods; remove lifecycle/receipt/registry platforms |
| Source | keep stable reader, one immutable object owner and reconciliation; fold admission/identity/current-row helpers into concrete owners |
| Preparation | keep materialization/unitization behavior; remove duplicate writers, adjacent reparse and receipt formats |
| Qdrant/publication | keep one SDK bridge and publication owner; fold view manager and whole-generation cleanup into publication; remove projection/proof/reclaimer scaffolds when callers move |
| Query | keep Exact and connected query/evidence/window behavior; remove public planner, executor, validator, handle and continuation authorities after useful code moves |
| Protocol/client/CLI | keep one framed protocol, owned child client, one daemon and one CLI; remove pairing/loopback/duplicate sessions |
| Optional workers/adapters | delete unless a selected source-backed consumer exists |

## Corrections to the historical map

- `search-retention` is **not a Cargo package on the inspected refactor branch**; main has not received that removal. Its documentation owns stopped-root local-object
  collection and archive/import. Online Qdrant cleanup belongs to publication.
- `search-revision-crypto` and Windows key code are optional encrypted-profile components only after #598's
  accepted handoff. They do not block the plain baseline.
- `search-os-secrets` does not become another general secret platform.
- `search-projection-planner` may be merged into publication/preparation if only bounded page construction
  remains.
- `search-epoch-pins` and `search-index-reclaimer` are removal targets after the small current-view manager and
  whole-generation cleanup move.
- optional document/model/MCP/overlay/enrichment packages cannot block the connected lexical product.

No row authorizes touching #598 private files. Coordinate only exact public overlap.
