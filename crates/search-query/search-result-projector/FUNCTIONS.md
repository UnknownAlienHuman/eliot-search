# search-result-projector — source map and deletion boundary

## Current API

| API/type | Actual behavior |
|---|---|
| `project_candidate_set(envelope, inputs, budget)` | Owns a complete `Vec`, re-sorts descending by supplied raw score and candidate ID, then applies result/source-byte ceilings after earlier validation/handle work. |
| `CandidateProjectionInput` | Carries already verified source bytes plus an already minted `SearchSourceHandle` and caller public metadata. |
| omission/coverage projection | Copies caller coverage and grows one omission record per rejected item. |
| `verify_handle_target_binding` | Compares supplied IDs only; it reads no handle/window/manifest owner. |
| callers | Normal product query caller was not found. The indexed process fixture constructs and calls it. |

The package is one 10,337-byte production file. It is downstream of disconnected planner/executor/validator
and handle systems; preserving its current API would preserve all of them.

## Replacement

Final ranking belongs to retrieval/fusion. Evidence lookup belongs to the pinned manifest and immutable
reader. Query-window lifetime belongs to one `QueryWindow`. Projection receives only final-order verified
items and remaining output limits:

```text
format_cards(
  items: &[VerifiedEvidenceItem],
  gaps: GapSummary,
  window_items: Option<&[WindowItemRef]>,
  context: &mut RequestContext,
) -> BoundedResponse
```

Rules:

1. never reorder `items`;
2. reserve/charge item and encoded-byte budget before copying excerpts;
3. include source/revision/representation/location and actual verified excerpt directly;
4. keep rank/contribution metadata separate from evidence identity;
5. sample finite gap details and retain aggregate counts;
6. ordinary ranked top-k remains explicitly non-exhaustive;
7. output failure does not advance the query-window cursor.

If these operations are simple response encoding, inline them into the daemon/protocol owner and delete this
crate.

## Delete after caller cutover

- old `SearchCandidateSet` envelope and handle-oriented result family;
- `CandidatePublicMetadata`, `SearchSourceHandle` input and target-binding helper;
- raw-score sorting/tie behavior;
- caller coverage/proof input and per-omission allocation;
- process fixture pipeline that treats oracle nominations as source-backed evidence;
- projector-specific compatibility/version adapters and source-shape fixtures.

## Verification

Check final order preservation, exact item/output ceilings, no retained-source read for unselected items,
bounded gap details, correct verified excerpt/location and zero query-window cursor advancement on partial or
failed output.
