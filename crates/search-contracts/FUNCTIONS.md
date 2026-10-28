# Shared data — current consumers only

This package keeps immutable data shared across real producer/consumer boundaries. It does not own query
planning, grants, runtime state or vendor schemas. [The implementation guide](../../docs/implementation/README.md#minimal-cross-owner-values)
owns cross-owner meaning; this recipe selects what can remain shared.

## Source seams

The [R055 source/caller inventory](https://github.com/UnknownAlienHuman/eliot-search/blob/44ee1661833c1139f1e0f1486f365a3a523fe814/crates/search-contracts/FUNCTIONS.md)
records the original baseline sizes and consumers. Recheck actual callers at the code cut; those counts are not
current runtime evidence. Old source families to replace with their consumers are:

| Family | Disposition |
|---|---|
| authority / recipes / query | Remove grants, eleven-recipe plans, legs, fingerprints and supplied proofs. |
| protocol / results / evidence / evidence_frames | Replace parallel V1/V2 families with current consumed DTOs. |
| lifecycle | Derive availability from constructed handlers, not readiness records. |
| indexed | Move physical SDK/schema/payload/query/readback types into the bridge. |
| IDs / bounds / canonical / digest / errors / source | Keep only real identity, wire bounds, locations and public error/gap semantics; remove unused taxonomies. |

## Shared values by delivery stage

Stage 2 needs only values crossing actual boundaries: source/revision/ObjectRef/representation identity,
generation-bound point/evidence references, selected physical/query profile references, supported locations,
finite Search input, source-backed cards/gaps and stable public error classes. A small owner-private value does
not belong here merely because its name resembles a domain concept.

Add multi-source selection with Stage 3A, Exact payload/results with the Exact handler, and WindowId /
WindowItemRef plus Continue/Expand payload/results with the live-session consumer. Unsupported operations fail
before effects; do not require their implementation to deliver baseline Search.

WindowItemRef is only the emitted `(window_id, item_index)` locator. The window privately stores ordered
EvidenceRefs into its pinned view; it must not store locators that refer back to itself as evidence.

Stage 2/3A requires no ViewSequence, validity interval or OccurrenceIncarnation. The conditional incremental
owner may add exact generation-bound interval/identity semantics only after Stage 3A budgets fail. PointId is
the backend address, not complete collision evidence. Use the [point recipe](../search-index-qdrant/search-point-identity/FUNCTIONS.md),
not a frozen indexed contract.

ExecutionContext, RequestContext, view guards, clocks/cancellation, streams, redb rows, publication progress,
query-window contents and vendor types remain with their concrete owners. No universal context or state bus.

## Serialization and identity

Use one current Serde DTO family; [protocol](../search-provider-protocol/FUNCTIONS.md) owns frame/message bounds.
Bound incoming frames before decode and encoded output before writing. Do not preserve pre-release byte order,
V1/V2 compatibility constructors or a third version family.

Keep hashes/canonical preimages only for real content/artifact/point/profile identity whose owner recomputes
and verifies it. Requests, plans, grants, state transitions and review records are not hashed into authority.

## Cutover

Change protocol/client/daemon/result consumers together for the selected operation. Move owner-private values
out; delete displaced records, reexports, codecs, goldens and generated coverage records with their last callers.
Do not preserve unused dependencies to avoid manifest churn. If the final shared set is trivial, merge it into
the protocol/application boundary and remove this workspace member.

After production code run minimal locked compiler/Clippy for changed real producers and consumers. Later
verification exercises current serialization bounds and source-backed output, not recipe counts or old wire bytes.
