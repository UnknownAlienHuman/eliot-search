# search-ports — removal target

`search-ports` is the global hexagonal-abstraction layer inherited from Architecture 8.4. It is not the
current application boundary and must not be expanded.

Read the [architecture manifesto](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md) and the current
[dependency graph](../../docs/handoff/DEPENDENCY_GRAPH.md).

## Verified public-main inventory

`src/` contains about **84.2 KiB** of production Rust:

- 23 public port traits and 80 registered operations;
- an 8.2 KiB generic `OperationContext`/mutation/receipt model;
- a 22.7 KiB conformance catalogue/fake engine;
- source, preparation, index, query, handle, process, secret and control interfaces in one package.

The package advertises a universal stable boundary, but caller searches show a different reality:

- `SearchIndexPort`, `AccessCompilerPort` and `SourceRevisionStorePort` have no real implementation in public
  main; matches are the conformance catalogue, historical documents and generated coverage maps;
- the complete `ControlJournalPort` implementation is the generic redb operation journal now being removed;
- many consumers import only `CancellationProbe` and `OperationContext` because old package rules required
  every effect to cross the same interface;
- daemon/provider/query paths still use concrete test oracles and parallel local APIs rather than these
  supposedly canonical ports.

This layer did not decouple the product. It created another contract family beside every concrete owner.

## Structural defects

### Relative deadline and opaque budget

`OperationContext` stores a relative millisecond duration and an opaque budget reference. A nested adapter can
start a fresh timeout from the full duration, while no shared counter actually limits nominations, reads,
retained bytes or output. The current implementation uses an absolute `Instant`, cancellation and concrete
remaining counters owned by one request/publication operation.

### Generic mutation identity and receipt

Every mutation is pushed toward `MutationIdentity` and `PortReceipt` even when the real correctness boundary
is one of:

- a short redb compare/set followed by direct typed readback;
- a content-addressed immutable object write;
- one durable `PendingPublication` page and actual Qdrant readback;
- process-local query/window work with no durable effect.

The generic receipt stores an operation ID, dependency digest, outcome and metadata but cannot establish the
actual database/object/Qdrant state. It encouraged receipt archives and replay maps in each subsystem.

### Interface catalogue as product evidence

`conformance.rs` freezes 23 traits/80 method names and scripted failures. Generated port maps then claim an
implementation owner even where no Rust implementation or connected caller exists. Passing the catalogue
cannot prove the retained-source → Qdrant → excerpt path.

### Wrong ownership direction

The global package owns interfaces for safe reading, source retention, preparation, Qdrant, access, exact
scan, handles, secrets and processes. Their semantics change with the concrete effect owner, so the shared
trait either becomes enormous or hides essential state behind associated types and opaque receipts.

## Replacement rule

Use concrete owner APIs by default:

```text
source acquisition   → concrete safe-reader/object-store methods
current state        → typed redb transactions and point/bounded-page reads
preparation          → direct materializer/unitizer functions
publication          → one Qdrant runtime + PendingPublication/cleanup rows
query                → ViewGuard + ResolvedScope + RequestContext + QueryWindow
protocol             → current typed request/result DTOs
```

A trait is justified only when a real consumer simultaneously needs more than one implementation. Then:

1. define the smallest trait beside the consumer;
2. use concrete current request/result types;
3. carry one absolute deadline/cancellation/counter context where blocking work exists;
4. return actual state/readback or a domain error, not a generic receipt;
5. keep production implementation and test fake local to that boundary;
6. delete the trait when the alternate implementation disappears.

Do not create a replacement `ports-v2`, service locator, adapter registry or generic effect bus.

## Useful pieces and destination

| Current piece | Destination |
|---|---|
| cancellation check shape | one application `RequestContext`; optional tiny local trait/atomic token |
| bounded page/stream limits | concrete query/inventory APIs which actually page |
| structured error classes | owner-specific errors and stable public protocol failures |
| Qdrant methods | private `search-qdrant-bridge` API with actual mutation/query/readback semantics |
| safe read/revision methods | concrete source/object owner |
| materialize/unitize/encode methods | direct preparation owners |
| clock/process/secret methods | concrete selected platform owner, only when used |
| scripted failure fakes | focused tests beside the consumer after the current API exists |

`PortReceipt`, generic `MutationIdentity`, `PackageOpaque`, the closed port registry and operation catalogue are
not retained merely to host these pieces.

## Cutover and deletion

Remove dependencies with the owner cutovers already scheduled:

- #270/#271/#330/#331 remove source/control/object/preparation port ceremony;
- #261/#263/#300/#302/#303 remove generic index/admin/pin/retention ports;
- #272/#274/#275/#279/#291/#293 remove access/exact/handle/query ports;
- #280/#332 remove provider/client use of generic operation context;
- optional model/overlay/process/secret interfaces disappear unless a selected current feature has a real
  consumer.

After the last real import moves:

```text
delete crates/search-ports/
remove workspace/dependency entries
remove docs/contracts/p00/PORT_OPERATIONS.md implementation authority
remove docs/handoff/PORT_CATALOG.md and generated ports coverage consumers
remove conformance/source-shape fixtures protecting the deleted trait names
```

Do not preserve adapters which translate a direct owner API back into the old traits. Compile and Clippy the
changed owners and immediate consumers; connected behavior is the proof.
