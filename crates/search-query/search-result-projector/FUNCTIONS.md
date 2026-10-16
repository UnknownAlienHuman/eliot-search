# Function contract — `search-result-projector`

**Status:** C26 owns bounded projection of private C24 validated candidates into the public
`SearchCandidateSet`. Root, handle, ranking-window and ledger integration remain separate acceptance
work under #294.

## `project_candidate_set(envelope, inputs, budget, encode_output)`

The projector validates each candidate's request and plan binding, candidate ID and C24 receipt, exact
source revision, emission permit and full source-owner fence. Only `ValidatedSearchCandidate` cards
enter the result.

Candidate order is the incoming C24 validated order. The projector does not compare raw scores. A
nonempty result requires one matching requested/executed coverage leg, and every C24 candidate must
carry the same owner-supplied leg ID. Any mixed-leg window fails closed until the actual opaque #278
ranking-window owner is available; a sole nonzero leg ID remains valid.

Candidate and source-byte budget omissions add a `CoverageGapKind::OmittedBudget` gap and a bounded
unknown, set the denominator to `Unknown`, and do not mint a continuation. Existing continuation
handles may pass through unchanged. Result membership coverage is reduced to emitted memberships;
source-lineage coverage is cleared when omissions cannot be mapped to retained cards.

The result receipt is computed with the C00 canonical BLAKE3 API under the full
`eliot/cbor/search-result-validation/v1` domain. It binds ordered candidate IDs, C24 receipts, handles,
owner/source identities, coverage and the complete result fence. Its `ReceiptRef` keeps the existing
tagged wire type. Raw source and query text are excluded.

`encode_output` is the parent-supplied exact provider encoder. It receives the completed result and the
configured byte ceiling, and returns bounded serialized bytes. The returned `ProjectedCandidateSet`
is non-cloneable and has private fields. Its `result()`, `omissions()` and `serialized_output_bytes()`
methods are read-only views; callers cannot replace the bytes checked by the projector. Emit those
exact bytes instead of serializing the typed result again.

The result receipt binds data only and grants no output authority. Root performs its fresh C18 check at
the actual frame boundary before emission.

## Ownership boundaries

- The projector consumes handles already minted by the handle owner. It has no handle table,
  expansion, authorization or revocation path.
- The projector does not create a substitute ranking-window owner or reorder cross-population scores.
- The provider/root caller must connect its exact encoder and emit the returned bytes. Until that
  consumer wiring exists, #294 is not complete.
