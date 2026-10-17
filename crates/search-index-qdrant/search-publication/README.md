# search-publication

Vendor-neutral publication admission, single-flight phase accounting and recovery
decisions. Effectful phases invoke injected effect/readback ports. C02 remains
the sole durable epoch/intent/visibility owner; C15 owns actual Qdrant effects.
This package has no concrete journal, Qdrant or platform implementation.

## Canonical admission (#260)

The primary owner is `search-publication`; the narrow C13 adapter exposes
canonical encoding/digest access. `search-unitizer` is the explicit
internal dependency needed to consume its opaque complete `VerifiedUnitSet`.
No concrete control, object-store, platform or Qdrant dependency is introduced.
The source/manifest/UnitSet/profile/currentness owners remain separate producers.

`validate_prepared_generation` is the sole admission function. Both normal
`PublicationCoordinator::submit` and `restore_inflight` call it before hydrating
an active slot, changing local epoch accounting or accepting a receipt prefix.
Requests provide private producer tokens through `PublicationAdmissionOwners`;
the production adapter must use actual preparation and coherent control/source
observations. It must not echo proposed fields, flags, digests or receipt labels.

Admission verifies the complete source/namespace/revision/materialization/
representation and membership scope, schema/profile/partition coordinates, every
opaque UnitSet occurrence and unit/reference digest, and C13 reconstruction of
canonical manifest bytes and point specifications. The actual current manifest
is independently checked by its owner. Captured owner state is checked again
before return. Wrong bytes, subsets, duplicate/foreign units, mixed scope,
changed physical IDs or changed guards return no accepted preparation.

The result is `PreparedGeneration` with private fields and read-only accessors.
Caller old/new digests and `preparation_receipt` are removed. C00 computes the
new `eliot/cbor/prepared-generation/v1` digest over the actual frozen inputs.
C13 alone owns manifest semantic digesting. The exact create/retain/retire diff
is computed once; stage, closure, combined readback, compensation, recovery and
retired-manifest emission borrow it. Retained points keep their exact original
readback shape and validity; creation binds to the admitted target epoch.

## Effect and readback boundary (#261.model)

`PublicationEffectPreparationOwners` supplies the actual planned vectors alongside
the admitted C13 products. Effect commands retain the full expected point identities,
payloads, vectors, validity and original operation context. `PublicationEffectPort`
performs an effect; `PublicationReadbackPort` supplies untrusted observations. The
coordinator compares the complete expected/read-back records before constructing
private stage, closure, control, snapshot or compensation proofs. A transport ACK
or matching caller digest cannot advance a phase.

An uncertain effect retains its original identity and blocks the next effect until
exact readback reconciles it. Visibility validates the actual CAS predecessor and
successor generations; intervening journal checkpoints need not be adjacent to the
original intent. Root still must provide real durable-control and Qdrant adapters.

## Empty targets and source completeness

`PublicationTarget::Retirement` is explicit. It requires the exact old verified
set and an independently observed retired membership, then derives a canonical
zero-entry C13 publication target. This is not a newly qualified empty source
representation. C09 raw-v3 rejects active empty input, and C13 rejects empty
unit sets; their unsupported cases remain explicit rather than fabricating an
empty UnitSet or using sentinel bytes. A retired target removes the current
manifest pointer on completion.

## Epochs and restoration

`PublicationCoordinator::new(max_points)` creates only finite, uninitialized
process-local state. Epoch getters return `None` until actual owner admission.
No caller constructor supplies accepted epochs or manifests. A validated normal
submission proposes exactly the next durable floor; its local reservation is
never reused. The Root adapter must commit the same target through C02 before
Qdrant effects. This package is neither another epoch ledger nor a root lease.

Restore re-admits original preparation through the same function, validates the
retained route/target/guards and receipt prefix, and never calls normal submit or
allocates another epoch. Changed pre-commit guards lead to compensation; bare
aborted/blocked records retain the occupied slot. Prior-process snapshot labels
never authorize current-process admission. Private effect-proof construction
belongs to this package. Actual journal-prefix readback and Native/Qdrant
execution remain #261/Root adapter work.

## Verification boundary

The canonical focused fixtures build real C08 and C09 products over an exact
in-memory revision and a C13 plan. Their owner ports are test-only observations;
they qualify neither storage, current policy nor Qdrant execution. The existing
recovery, abort and restore fixtures under `tests/` use the same admission API.
Their collision cases rebuild valid C13 products before checking immutable
point conflicts; malformed manifests are tested separately.

There was no product caller of the removed boundary. Root producer/normal
consumer wiring, full submit/restore qualification and installed/scale evidence
remain required. In-memory inputs are intentionally replaced directly; existing
C02 persisted codecs are unchanged. New prepared-input identity requires rebuild
rather than relabelling an old caller-shaped preparation.
