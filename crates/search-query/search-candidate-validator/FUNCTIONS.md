# Function contract — `search-candidate-validator`

**Status:** W4/P08 source-backed validation contract; security hardening continues in W7/P13.

A nomination can become evidence only after exact source readback and live-fence validation. Qdrant
payload text, cached snippets and current-path bytes are never evidence substitutes.

## Operations

### `precheck(nomination, plan, live_state, overlay_state) -> ValidationPrecheck`

Checks request/plan binding, projection membership, collection/profile identity, epoch validity,
overlay shadow, current grant/live deny/purge and candidate budget before any source read.
`CONTAMINATED_LEG` is returned when the scoring population changed restrictively.

### `build_readback_request(nomination, plan) -> Result<ExactRevisionReadbackRequest, ValidationError>`

Requests the exact source revision, representation/unit, native anchor, optional fixed excerpt digest,
profile and assurance expected by the nomination. It cannot substitute the latest path/revision.

`expected_excerpt_digest = Some(digest)` retains the legacy exact-digest requirement.
`expected_excerpt_digest = None` is accepted only by the versioned raw-v3 full-unit entrypoint below;
the legacy `validate` and `reopen_and_verify` paths refuse it before source readback.

### `reopen_and_verify(request, readback_port) -> Result<VerifiedSourceSlice, ValidationError>`

Verifies source/revision IDs, content digest, byte length, residency authorization, coordinate/loss map,
anchor bounds, unit/extractor/profile identity and requested disclosure ceiling.

### `validate_raw_v3_full_unit_v1(nomination, context, candidate_id, source_membership, source_revision, source_revision_ref, source_owner_fence, verified_unit_set, max_read_bytes, readback_port, final_live_state) -> ValidationOutcome`

Requires the caller's typed C09/C03 source and membership records, the actual `CandidateId`, and the
owner-verified complete raw-v3 unit set. It compares membership/source/revision, namespace binding,
content digest and length, residency, representation/profile, and exact unit range before invoking
the readback port. The verifier hashes returned unit bytes, derives the exact-byte excerpt digest,
rechecks the final live fence, then creates an immutable candidate with a canonical BLAKE3
`candidate-validation:v1` reference. Receipt computation failure produces a gap; it has no
empty or caller-supplied fallback.

The lower-level `reopen_and_verify_raw_v3_full_unit_v1` verifies a byte slice against its request but
does not produce a qualified candidate or validation receipt. The metadata-free `validate` entrypoint
returns `VALIDATION_OWNER_METADATA_REQUIRED` without reading source bytes.

### `ValidatedSearchCandidate` read-only accessors

Candidate fields are private. Consumers read `candidate_id()`, `leg_id()`, `raw_score()`, `source()`,
`emission_permit()`, `source_revision_ref()`, `source_owner_fence()`, `source_owner_generation()`,
`object_residency_key_digest()`, and `candidate_validation_receipt_ref()`. The verified slice exposes
read-only profile and residency getters in addition to its source/range/hash getters.

### `recheck_before_emission(verified, plan, live_state) -> Result<EmissionPermit, ValidationError>`

Revalidates binding, grant, owner generation, source/workspace view, live deny, purge, shadow and
residency immediately before projection.

### `validate(nomination, context, ports) -> ValidationOutcome`

Returns exactly one tagged outcome:

```text
Validated(ValidatedSearchCandidate)
Gap(CandidateValidationGap)
ContaminatedLeg(ReplanSignal)
```

A gap contains reason/scope/freshness metadata only: no excerpt, source handle or evidence-bearing
payload. Invalid outcomes cannot be inserted into the validated candidate type.

### `material_coverage_change(before, after, plan) -> CoverageChange`

Determines whether lost nominations require bounded refill/replan or explicit incomplete coverage. It
never relabels remaining top-k as complete.

## Cancellation, retry and failure

Readback is bounded and cancellable. Cancellation/timeout/unreadable/mismatch yields a gap, never a
partially validated candidate. Retrying identical captured inputs is read-only and safe. Concrete
revision/CAS/Qdrant types remain behind ports.

## Required fixtures

Stale payload cannot be cited; exact digest/length/anchor/profile mismatch; overlay shadow; unreadable
revision gap has no evidence; revocation/purge at precheck/readback/emission; contaminated-leg signal;
loss-triggered bounded refill versus explicit gap; lossy coordinate map never claims raw exactness; fake
readback proves adapter independence.
