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

Requests the exact source revision, representation/unit, native anchor, excerpt digest, profile and
assurance expected by the nomination. It cannot substitute the latest path/revision.

### `reopen_and_verify(request, readback_port, context) -> Result<VerifiedSourceSlice, ValidationError>`

Verifies source/revision IDs, content digest, byte length, residency authorization, coordinate/loss map,
anchor bounds, unit/extractor/profile identity and requested disclosure ceiling.

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
