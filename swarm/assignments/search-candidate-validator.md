# `search-candidate-validator` implementation packet

**Path:** `crates/search-query/search-candidate-validator`  
**Capability:** C24  
**Delivery:** W4 / P08; security hardening W7 / P13  
**Gate:** BLOCKED until access, ports and source-readback contracts are accepted  
**Trace:** S14.4, S15, S23.1, H13-H14, P08, P13

## Mission

Turn nominated candidates into either `ValidatedSearchCandidate` or a non-evidence
`CandidateValidationGap`.

## Owns

- live deny/purge checkpoints;
- projection membership and overlay-shadow checks;
- exact revision reopen and digest/length/anchor/profile verification;
- stale/unreadable/inaccessible rejection and replan signal.

## Must not own

- Qdrant payload text as evidence;
- candidate-only cleanup of a contaminated rank leg;
- client admission;
- current-path substitution for a fenced revision;
- concrete store/index/process dependencies.

## Logical operations

1. `validate(candidate, fence, live_state, readback) -> ValidationOutcome`
2. `reopen_and_verify(handle, readback) -> Result<VerifiedSourceSlice, ValidationError>`
3. `validate_anchor_and_unit(source, anchor, expected) -> Result<(), ValidationError>`
4. `material_coverage_change(before, after) -> bool`

```text
ValidationOutcome =
  Validated(ValidatedSearchCandidate)
  | Gap(CandidateValidationGap)
  | ContaminatedLeg(ReplanSignal)
```

## Invariants

- every emitted candidate is reauthorized and source-backed;
- invalid nominations never enter the evidence candidate list;
- a validation gap carries no excerpt/evidence handle;
- revoked populations contaminate the whole influenced scoring/IDF leg;
- concrete readback remains behind accepted ports.

## Exit evidence

- stale/unreadable candidate cannot be cited;
- digest/length/anchor/profile mismatch becomes a gap;
- revocation/purge blocks output at every checkpoint;
- overlay shadow rejects stale base;
- contaminated leg requests discard/replan;
- gap serialization contains no evidence bytes or evidence-bearing handle;
- fake readback proves store independence.

Target `src/` ≤7,500 lines; split review before 8,500 total; hard stop at 10,000 including local tests.
