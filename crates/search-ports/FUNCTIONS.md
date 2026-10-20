# search-ports — source/caller audit and deletion map

`search-ports` is the global abstraction catalogue inherited from Architecture 8.4. It is not the current
application boundary. This file records the exact public-main census and the cutover required to delete it.

## Measured source

At `c8e38b6`, `src/` contains **84,157 bytes (~84.2 KiB)**:

| Source | Bytes | Current role |
|---|---:|---|
| `conformance.rs` | 22,700 | freezes 23 traits, 80 methods and scripted fake outcomes |
| `source.rs` | 10,037 | admission/inventory/ownership/safe-read/revision/residency interfaces |
| `context.rs` | 8,172 | relative deadline, cancellation, opaque budget, mutation identity and generic receipt |
| `query.rs` | 7,867 | access, overlay and exact-scan interfaces |
| `index.rs` | 6,124 | search/index/admin/pin interfaces |
| `runtime.rs` | 5,984 | clock, secret and process interfaces |
| `stream.rs` | 5,895 | generic finite stream/page helpers |
| `handles.rs` | 5,763 | generic handle-store interface |
| `preparation.rs` | 3,612 | materializer/unitizer/enricher/lexical interfaces |
| remaining files | 8,003 | reexports, optional model interface and support |

Repository search finds **35 Cargo manifests** depending directly on `search-ports`. The generic vocabulary
also fans out widely: about 90 search hits for `OperationContext<`, 37 for `MutationIdentity`, 14 for
`PortReceipt` and 13 for `FakeCancellation`. These counts are migration inventory, not proof that the types
are useful.

## Exact implementation census

### Implemented only by a subsystem being deleted

| Port | Implementation | Disposition |
|---|---|---|
| `ControlJournalPort` | `BoundControlJournal` in `search-control-redb` | delete with permanent operation receipts/full journal replay |
| `ControlSnapshotPort` | the same `BoundControlJournal` | delete with full copied `ControlSnapshot` publication |

These implementations do not justify the shared traits. The replacement is direct typed redb current-state
operations and narrow point/bounded-page reads.

### Adapter exists, but no product caller was found

| Port | Adapter | Finding |
|---|---|---|
| `SourceInventoryPort` | `RegistryInventoryAdapter` | wraps a complete cloned registry snapshot, access/currentness maps and `FakeCancellation`; search finds definition/reexport only |
| `SourceOwnershipPort` | `RegistryOwnershipAdapter` | explicitly process-local/non-authoritative, uses `FakeCancellation`; search finds definition/reexport only |

Both adapters belong to the old portfolio/corpus/namespace-cutover stack. Do not connect them to the new
application. Current source rows and one immutable `PublishedView` replace them.

### Declared without a real implementation in public main

Searches for `impl <Port> for` found no implementation for these **19** traits:

```text
ClockPort
SecretStorePort
ProcessSupervisorPort
SourceAdmissionPort
SafeReaderPort
SourceRevisionStorePort
ResidencyPolicyPort
MaterializerPort
UnitizerPort
CodeEnricherPort
LexicalEncoderPort
ModelProviderPort
SearchIndexPort
SearchIndexAdminPort
EpochPinPort
AccessCompilerPort
OverlayPort
ExactScannerPort
HandleStorePort
```

Matches are the conformance catalogue, historical contracts, generated maps and assignment documents. Those
records frequently claim an “implementation owner” which has no Rust implementation of the advertised trait.
Deleting the generated governance is therefore part of deleting the false boundary.

## Why the common context is wrong

`OperationContext` carries a **relative** millisecond duration and an opaque budget reference. Nested owners
can restart the full duration, while the opaque budget performs no shared accounting of nominations, source
reads, retained bytes or output.

Replace it with operation-specific state:

```text
RequestContext {
  absolute monotonic deadline
  cancellation token
  remaining nominations/items/read bytes/result bytes/output bytes
  PublishedViewGuard + ResolvedScope where query needs them
}
```

Publication uses its own absolute context plus the durable pending page/cursor. Object/source/control
operations receive the same deadline/cancellation when they block, but do not inherit query-only counters.

## Why generic mutation receipts are wrong

`MutationIdentity` and `PortReceipt` reduce unrelated effects to operation ID, dependency digest, generic
outcome and metadata. They cannot establish the real postcondition:

- redb commit uncertainty is resolved by reading the affected typed row;
- immutable object publication is resolved by exact final-object verification;
- Qdrant write uncertainty is resolved from the persisted publication page and actual point/schema readback;
- query, paging and pure preparation have no durable mutation to receipt;
- child/process uncertainty is resolved through the actual owned process/handle state.

Remove generic receipts and replay maps. Keep durable state only where a real effect can outlive the process.

## Concrete replacements

| Old port family | Replacement owner/API |
|---|---|
| clock/cancellation | `std::time::Instant` plus one small local cancellation token; test clock only beside a consumer that needs it |
| secret/process | concrete selected platform owner; no interface until two real implementations exist |
| control | typed redb transactions and direct reads |
| admission | small pure source-policy function called directly by ingestion |
| inventory/current source | typed root/source/head rows and bounded inventory snapshot |
| safe read / revision store | concrete stable-reader and one immutable object writer/reader |
| residency | selected storage profile in the object owner; plain path invokes none |
| materialize/unitize/enrich | direct preparation functions over current types |
| lexical | Qdrant native BM25 `Document`; bounded text expansion is a direct helper |
| model | no interface until one selected model has real preparation/query consumers |
| search/index/admin | private Qdrant runtime with exact methods used by publication/query |
| epoch pins | `PublishedViewGuard` / `Arc` lifetime owned by publication/query windows |
| access | `resolve_scope(requested_ids, pinned_view, optional_session)` |
| overlay | no baseline owner; later thin map behind a real editor integration |
| exact scan | direct bounded retained-source operation |
| handles | one concrete process-local `QueryWindow` |

A trait may survive only beside a real consumer that simultaneously needs multiple implementations. It must
use current concrete values and return actual state/error, not a generic receipt.

## Cutover order

1. Replace `ControlJournalPort`/`ControlSnapshotPort` while implementing typed current redb state.
2. Remove `SourceInventoryPort`/`SourceOwnershipPort` with registry replay, portfolios and namespace cutover.
3. Move source/object/preparation callers to concrete current APIs; remove unused Cargo dependencies.
4. Connect publication/query directly to the one Qdrant runtime and view manager; delete index/admin/pin/
   access/handle/exact interfaces.
5. Replace provider/client generic context with the current framed protocol and direct handler context.
6. Remove optional process/secret/model/overlay traits unless a selected feature has two real implementations.
7. Delete `conformance.rs`, fake cancellation/receipt matrices, generated port maps and P00 port contracts.
8. Delete `crates/search-ports/` and workspace dependency after the final import disappears.

Do not create `search-ports-v2`, forwarding adapters, a service locator or an effect bus.

## Tests

Delete conformance counts, scripted fake-operation matrices, generated port ownership maps and source-shape
tests protecting trait names. Move only causal behavior into the concrete owner:

- current-row compare/set/restart;
- object write/reuse/readback;
- preparation coordinates;
- Qdrant mutation/query/readback;
- view lifetime and cleanup;
- query deadline/counters/window output atomicity;
- protocol partial I/O and one terminal response.

Compile and run strict Clippy only for the changed owner and immediate real consumers during each cutover.
The connected source → Qdrant → retained excerpt path, not a port catalogue, proves the architecture.
