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

### `validate_raw_v3_full_unit_v1(nomination, context, readback_port, final_live_state) -> ValidationOutcome`

Uses the same nomination precheck, source metadata/range/residency/profile checks and final live-fence
recheck as `validate`. The caller must bind the request to the current accepted profile and supply
the complete byte range of one verified raw-v3 unit. The validator checks the v3 profile-ID shape
and returned bytes, but cannot establish profile acceptance or whole-unit membership from the
request alone.
The verifier recomputes the v3 unit-content digest through `search-unitizer` from the returned bytes,
compares it with `expected_unit_digest`, and computes the separate exact-byte excerpt digest through
`search-contracts::blake3_exact_bytes`. A supplied excerpt digest is compared as an additional
expectation; absence derives the digest only after full-unit byte verification. This entrypoint cannot
prove from its request alone that a caller-selected subrange is a complete unit.

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
