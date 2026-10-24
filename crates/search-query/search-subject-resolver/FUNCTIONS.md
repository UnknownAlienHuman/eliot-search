# search-subject-resolver functions

## Current function map

These functions exist in [src/lib.rs](src/lib.rs). `resolve_subject` currently has unit-test callers;
no production source/index candidate producer is connected through this API.

| Function | Actual behavior |
|---|---|
| `rank_resolution_basis` | Map a supplied `MatchBasis` to an optional `ResolutionPriority`. |
| `validate_resolution_context` | Check supplied request/context fields and disclosure, currency and observation flags; it does not resolve scope or read source storage. |
| `resolve_subject(request, context, steps, limits)` | Bound/order supplied `ResolutionStep` candidates and produce resolved, ambiguous, not-found, scope-empty or incomplete output. A missing/incomplete applicable higher-priority step currently blocks later resolution. |
| `collapse_equivalent_occurrences` | Validate candidate fields, reject duplicate candidate digests and group by supplied hypothesis digest when an equivalence receipt is present; otherwise group by candidate digest. It does not establish alias/rename equivalence. |
| `build_ambiguity_set` | Sort and bound supplied candidate hypotheses, retaining differentiation and omission information. |
| `issue_resolution_receipt` | Assemble the existing resolution/context/candidate metadata into a receipt; it does not produce source evidence. |
| `revalidate_resolution` | Compare the receipt with supplied live-state fields and classify whether resolution must change. |

There are no separate implemented `normalize_subject_request`, `resolve_explicit_reference`,
`resolve_qualified_key`, `resolve_exact_name`, `resolve_signature_and_kind`,
`resolve_structural_lexical_candidates` or `assemble_resolution` APIs. Do not implement that historical
function list merely to complete a contract.

## Connect and simplify

Follow the [README's connected behavior](README.md#minimal-connected-behavior): actual source/range or
symbol lookup, then bounded lexical nomination when needed. Prepared syntax/SCIP data supplies the
candidates; existing source validation supplies the bytes and scope checks. A receipt or boolean label
cannot replace either producer.

Reuse the useful selection, ordering and ambiguity logic in that query path. Remove the mandatory
ladder materialization, receipt/revalidation machinery and unnecessary candidate copies with their
last consumers. Keep explicit location failures, material ambiguity and truncation visible. A renamed
analogue is a comparison candidate; it does not become the same local entity. No-match here remains a
result of the executed lookup, not a complete source-wide absence claim.
