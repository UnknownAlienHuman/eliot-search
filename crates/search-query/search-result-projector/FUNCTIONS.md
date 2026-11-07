# search-result-projector — source map and deletion boundary

## Current consumer

The former crate/manifest/source and exclusive fixtures are removed. Current ranked evidence metadata is
formatted in [verified_query::metadata](../../../bins/eliot-searchd/src/verified_query.rs); the same query worker
verifies retained bytes/ranges before building text-bearing hit items; gaps preserve their actual causes.
Protocol/CLI encoding preserves their final
order. The policy below does not expose or require the retired projector API.

## Retired API description (historical)

| Former API/type | Historical behavior |
|---|---|
| `project_candidate_set(envelope, inputs, budget)` | Owns a complete `Vec`, re-sorts descending by supplied raw score and candidate ID, then applies result/source-byte ceilings after earlier validation/handle work. |
| `CandidateProjectionInput` | Carries already verified source bytes plus an already minted `SearchSourceHandle` and caller public metadata. |
| omission/coverage projection | Copies caller coverage and grows one omission record per rejected item. |
| `verify_handle_target_binding` | Compares supplied IDs only; it reads no handle/window/manifest owner. |
| callers | Normal product query caller was not found. The indexed process fixture constructs and calls it. |

The retired package was described as one 10,337-byte production file downstream of the disconnected
planner/executor/validator and handle systems. That size/API description is historical, not current source presence.

## Replacement

The following semantic sketch describes the projection boundary, not an implemented public API.
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

This boundary builds structured evidence, not terminal-ready strings. Preserve exact excerpt values and source
coordinates; the [CLI presentation boundary](../../../bins/eliot-search/FUNCTIONS.md#read-only-search) owns safe display
escaping and its expanded-byte limit. JSON escaping preserves the decoded value; lossy display text never becomes
stored evidence or an expansion locator. A verified digest does not authorize terminal control sequences.

These operations are inlined in the current daemon/protocol consumer; the former projector crate was removed.

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
