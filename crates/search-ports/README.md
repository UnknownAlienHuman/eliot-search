# search-ports — delete the global abstraction catalogue

This package is not the application boundary. At public main it contains about **84.2 KiB** of production
Rust, **23 public traits**, **80 registered operations**, a generic relative-deadline/receipt model and a
22.7 KiB conformance/fake catalogue.

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md), [exact source/caller census](FUNCTIONS.md) and
[test cutover](../../docs/implementation/TEST_CUTOVER.md).

## Verified result

The advertised universal boundary does not exist in the running product:

- **19 of 23 traits have no public-main implementation**;
- `SourceInventoryPort` and `SourceOwnershipPort` have unused adapters built on `FakeCancellation` and old
  registry/cutover state;
- `ControlJournalPort` and `ControlSnapshotPort` are implemented only by the generic journal/snapshot system
  now being removed;
- **35 Cargo manifests** depend on the package, largely because historical rules forced every effect through
  the same context/receipt vocabulary;
- generated maps claim exact implementation owners even when no Rust implementation exists.

A package dependency, conformance descriptor or fake is not a working abstraction.

## Replacement rule

Use concrete owner APIs by default:

```text
source acquisition   → stable reader + immutable object owner
current state        → typed redb rows/transactions
preparation          → direct materializer/unitizer functions
Qdrant                → one private concrete bridge/runtime
publication           → PendingPublication + current view + cleanup
query                 → PublishedViewGuard + ResolvedScope + RequestContext
paging/expansion      → one QueryWindow
protocol              → current typed request/result records
```

A trait is justified only when one real consumer currently needs multiple implementations. Then define the
smallest trait beside that consumer, use current concrete values, return actual state/error and keep its fake
local. Delete it when the alternate implementation disappears.

Do not create `search-ports-v2`, a service locator, adapter registry, generic effect bus or forwarding crate.

## Context and effect semantics

The current `OperationContext` stores a relative duration and opaque budget. Replace it with an absolute
`Instant`, cancellation and real remaining counters owned by the request/publication operation. Nested calls
must not renew time.

The current `MutationIdentity`/`PortReceipt` cannot establish any concrete postcondition. Resolve effects at
their owner:

- redb uncertainty → read the affected typed row;
- object publication → verify the exact final object;
- Qdrant uncertainty → inspect exact pending page/schema/point state;
- query/paging/pure transformations → no durable receipt;
- process operations → inspect the actual owned process/handle.

## Deletion

The owner cutovers remove imports in this order:

1. typed control state replaces journal/snapshot ports;
2. current source rows replace inventory/ownership adapters;
3. source/object/preparation use direct functions;
4. publication/query use the concrete Qdrant/view/window owners;
5. current protocol removes generic provider context;
6. optional process/secret/model/overlay interfaces disappear unless selected real consumers require them;
7. conformance/fakes/generated port maps/P00 port contracts are deleted;
8. the workspace member and final dependency entries are removed.

Move genuinely useful small types to their natural owner. Do not keep old traits merely to reduce Cargo.toml
churn or preserve obsolete tests.

## Verification

Use minimal Rust 1.98 check/strict Clippy for each changed owner and immediate real consumers, plus focused
behavior cases. Delete trait-count, fake-failure and source-shape fixtures. The connected application—not a
catalogue of interfaces—proves the boundary.
