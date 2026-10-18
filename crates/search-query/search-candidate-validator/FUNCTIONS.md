# Functions — search-candidate-validator

This owner turns a candidate nomination into a verified source slice or an explicit gap.
Use [query integration](../../../docs/implementation/README.md#turn-a-point-into-evidence) for the
real manifest/readback handoff. Existing grant, fingerprint and residency fields are source facts,
not immutable requirements for every local query.

## Implemented API

All operations below exist in [src/lib.rs](src/lib.rs).

| Function | Current behavior |
|---|---|
| `precheck(nomination, context)` | Checks request/plan identity, candidate limit, allowed membership, collection/epoch, shadowed unit and current live-access state. Returns `Proceed`, `Gap` or `ContaminatedLeg`. |
| `build_readback_request(nomination, max_read_bytes)` | Rejects empty/out-of-bounds ranges and copies the exact revision/unit/profile/digest expectations into `ExactRevisionReadbackRequest`. |
| `reopen_and_verify(request, port)` | Calls `RevisionReadbackPort::read_exact`; compares returned identity, digest, profile, residency, assurance, coordinate and length fields. Returns `VerifiedSourceSlice` on agreement. |
| `recheck_before_emission(verified, request_security, live_security)` | Calls the existing live-access check and binds its permit to the verified source membership/revision. It does not independently reload a grant, source catalog or root owner. |
| `validate(nomination, context, max_read_bytes, port, final_live_security)` | Composes those steps and returns `Validated`, `Gap` or `ContaminatedLeg`. |
| `material_coverage_change(before_validated, after_validated, target_candidates, refill_budget_remaining, contaminated_leg)` | Selects unchanged, bounded refill, replan or explicit incompleteness after candidate loss. |

The inspected daemon tree uses a `FixedReadback` implementation in process tests; production retained
readback is not connected. These functions do not make Qdrant candidates source-backed by themselves.

## Finish the actual source handoff

Resolve a nominated point through the pinned published manifest, then the unit/representation metadata,
to its retained content revision and exact range. The executor's point ID, membership, score and digest
fields alone are insufficient. The query/source owners provide this lookup using their actual records;
do not manufacture UUIDs, cast source-record versions into content revisions or read the latest pathname.

`reopen_and_verify` currently compares supplied digest fields; it does **not** hash
`SourceReadback.bytes`. One real owner must verify the actual object bytes and requested range, either
in the retained reader or in this validator. Expected digests copied from the request alongside
arbitrary same-length bytes cannot establish evidence. Preserve the actual source/representation
coordinate connection and report missing or mismatched data as a gap.

The current readback method has a byte ceiling in its request but no explicit deadline/cancellation
parameter. Connect the runtime's request context through the reader when integrating; a trait name
does not make a blocking read cancellable. Bound real I/O and output, and keep partial or failed
readback out of successful evidence.

## Simplify with the real consumers

Keep one validation path. Remove unused plan fingerprints, residency proofs and mandatory grant inputs
with their producers/consumers; do not add a second validator or satisfy old checks with synthetic
`allowed` values. The trusted-local scope still excludes unselected sources, and actual removal or
restriction must invalidate affected results. A selected restricted-client profile additionally
enforces real delegated scope and current revocation at its admission/emission boundaries.

A Qdrant score/payload, supplied permit or a successful constructor is not source evidence. Missing
retained bytes yield an explicit unavailable/stale result. Ordinary ranked output remains
non-exhaustive after validation or refill; only an explicit exact operation can claim completeness.

## Behavior to check

Use the connected reader and real manifest/unit records to check changed bytes, wrong revision/range,
profile/coordinate mismatch, missing objects, excluded or removed sources and cancellation.
Check restricted-client revocation when that profile is selected. Test that rejected candidates emit
no excerpt and candidate loss cannot turn top-k into an exhaustive result. Mocks exercise pure
branches; they do not prove retained storage, byte verification or runtime integration.
