# search-source-admission-receipt

Pure v2 receipt issuance and current-policy verification over immutable models
from `search-source-admission`. This package owns the full receipt implementation
and its causal fixtures; callers import `receipt_v2` from this permanent leaf.

`issue_receipt_v2` checks the immutable decision's policy fence, revalidates its
observation, checks raw lower ceilings and counts before variable copies, and
computes the receipt through the shared checked CBOR/BLAKE3 producer.
`verify_receipt_v2` checks schema, current policy fence and retained identities,
revalidates the observation and recomputes the exact receipt before returning
the private immutable verified wrapper. Refusals return no partial receipt.

The original caller supplies checkpoints; cancellation and its existing deadline
are never renewed. Failures contain closed metadata. There is no I/O, native
identity, root/source registration, access/currentness proof, v1 fallback, mutable
receipt field or caller-supplied digest/accepted constructor.

#424 moves the complete accepted implementation from admission with its fixtures.
The old `search_source_admission::receipt_v2` module is deleted. New callers use
`search_source_admission_receipt::receipt_v2`; receipt profile/domain/bytes and
existing v1 behavior remain compatible. Ingestion/currentness adoption remains
#270/#271, and whole-call batch evaluation stays in admission under #241/#405.

Existing receipt fixture groups remain authored, not run. Production source gates
and independent exact-head review are separate from runtime/native/live-Qdrant/
resource/scale/installed qualification under #215. See `RECEIPT_V2.md` for the
unchanged receipt schema and operation ceilings and `AGENTS.md` for ownership.
