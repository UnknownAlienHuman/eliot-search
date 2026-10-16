# Function contract — `search-handles`

## Canonical storage

The package stores `search_contracts::lifecycle::SearchSourceHandleRecord` in one private
`HandleStore`. The token index is keyed only by `HandleTokenDigest`; plaintext token bytes exist only in
the returned `SearchSourceHandle` and the transient crypto result. `RetainedSourceTarget`,
`EphemeralHandleTarget`, and the lifecycle record variants are re-exported from C00 without a second
public target or record shape.

Package-private supplemental fields retain the source-membership invalidation key, representation/unit
identity, owner epoch, disclosure ceiling, and a process-local source-protection guard that C00's
ephemeral target does not carry. They are never accepted back as caller authority.

## Minting

### `mint_ephemeral(target, source_membership_id, binding_id, grant_id, owner_epoch, disclosure_ceiling, crypto, cancelled)`

Mints an unsaved-buffer handle with the actual membership resolved by the authenticated buffer owner.
That membership is mandatory private supplemental metadata, so exact membership invalidation and both
deny/purge selectors include unsaved handles. It is never inferred from a source ID, workspace or
buffer snapshot. A retained-source variant fails closed here; the C00 unsaved target is unchanged.

### `mint_ephemeral_retained_source(target, source_membership_id, representation_id, unit_id, binding_id, grant_id, owner_epoch, disclosure_ceiling, protection, crypto, cancelled)`

Mints a memory-only retained-source handle only after the supplied source owner returns an opaque guard
for the exact C00 target, with protection lasting through the minted expiry. The store recomputes the
token digest through `HandleCryptoPort` and rejects provider material whose digest does not match its
plaintext token. No source-protection implementation is included here; callers without the C17 owner
guard cannot mint this handle class. Durable mint remains unavailable until its real source-retention
lease producer exists.

## Expansion

### `HandleStore::expand(handle, requested_max_bytes, crypto, authorization, readback, deliver)`

The store hashes the supplied token, resolves its current private record, and validates its ID,
revision, class, and active state. The request names the record's original C00 `NativeAnchor`; it has
no caller-supplied start/end range to widen.

`HandleReauthorizationPort` creates opaque checkpoints at `BeforeSourceReadback` and `BeforeOutput`.
`HandleReadbackPort` reads the exact target and verifies an opaque source-owner observation. The package
checks selected byte-range length against the shared C00 hash ceiling before I/O, limits byte-range
readback to that exact length, and requires the returned byte count to match. Native coordinate ranges
retain their exact owner-verified anchor and use the shared hash ceiling as their readback cap. The
package recomputes the excerpt digest with a limit derived from the validated returned byte count and
compares it to the C00 target. A wider policy/request ceiling does not reject a small exact range.
Delivery runs while the store borrow and fresh output checkpoint remain live. No caller-provided
record, validation context, digest, or permit can authorize expansion.

The parent daemon must implement the authorization and readback ports using current C18 checks and the
exact C07/root source owner. It must also provide the C17 protection guard used at mint. Those adapters
are outside this package scope.

## Lifecycle

`invalidate` and `expire` update the canonical C00 record status and drop any in-memory protection
guard. Their receipts describe local handle-state progress only; they do not claim completion of an
external source-retention cleanup.

Invalidation and expiry remain batch-bounded. Token lookup and the handle-ID index stay synchronized.
Terminal-record ceilings and compaction remain #282 work; this patch does not close that lifecycle gate.
