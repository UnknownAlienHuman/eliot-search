# Evidence resolution — one retained source, explicit coordinate domain

This file owns the actual excerpt and failure meaning; [windowing](../search-continuation/FUNCTIONS.md) owns
page progress. Inspected `src/lib.rs` through `validate` at `e5350809` on 2026-10-23. This is a replacement recipe,
not a claim that the normal product path uses the real retained reader.

## Existing API and its limits

| Source | Inspected behavior |
|---|---|
| `precheck` / `build_readback_request` | Compare public caller metadata and copy expected revision/profile/digest/range values from the nomination. Candidate ordinal and read limits are supplied per call. |
| `reopen_and_verify` | Compares returned digest fields but does not establish their relation to returned bytes. Every port error becomes `SourceUnreadable`. It requires `coordinate_map_exact` and `bytes.len() == byte_end - byte_start`. |
| `validate` | All read/verification errors become a candidate Gap. There is no separate request-stopped outcome for a port cancellation/deadline. |
| `recheck_before_emission` / coverage refill | Caller-shaped permits and count-based decisions, not a real source or completeness observation. |

The single-range length rule can describe a raw byte slice; it cannot describe transformed text as if its
length were a raw-source range. For example, UTF-16LE `41 00` maps to UTF-8 `41`: source span length 2, displayed
span length 1. Feeding transformed bytes through that raw-only contract is rejected; relabelling its offsets
would instead produce a false citation. This is an API/design counterexample, not a reproduced product failure.

Preserve useful validation, not public proof-shaped structs or the inability to distinguish failure causes.
The old daemon exact-match helper in `direct_preparation/spine.rs` separately checks character boundaries
and literal equality; its completeness check accepts honest partial results. Do not delete correct behavior
merely because its name contains `gate` or because the old architecture wrapped it in receipts.

## Direct resolution

```text
ranked hit -> selected pinned source/evidence row -> real retained object/representation -> excerpt
```

Reject foreign route/generation/scope or contradictory stored point identity before source reads. The source
ID in Qdrant is a lookup hint, not evidence authority: resolve the exact `(source_id, point_id)` key in the
pinned table selected by the control owner. Do not fall back to another source or the latest view on a miss.
The authoritative entry supplies retained references, revision, representation and ranges.

Select the bounded candidate page first. Use the one request deadline/cancellation and remaining read/output
limits; reserve before calling the real reader, which enforces actual I/O/allocation limits. Group selected
reads from the same object when useful. Verification follows the object's actual content/address/integrity
format, not digest fields echoed by a port. A verified reader need not rehash a whole object per excerpt.

## Source bytes and represented text are different coordinate spaces

Keep the source/preparation owner's existing maps; no new mapping framework or universal document model.

- A raw-source slice is checked against the retained raw object and its raw-byte bounds. Only this form may
  equate excerpt byte length with the source interval length without another transform.
- A supported decoded/normalized/extracted excerpt is checked against its exact retained prepared representation
  and that representation's bounds. Resolve its source/page/region relation through the associated map and
  transform records. Check each domain against its own object; do not require equal byte lengths.
- A normalized or extracted excerpt remains source-backed with the declared mapping/loss. Do not upgrade it to
  exact original bytes. Non-contiguous, approximate or unmapped regions remain explicit; an enclosing range
  cannot be presented as an exact contiguous original slice when it is not one.

A card carries the actual excerpt domain/representation and truthful source location. Character-safe clipping
happens in that representation; obtain the clipped location through its map rather than adding displayed UTF-8
length to a raw offset. Reuse existing page/line/map metadata. Unsupported mapping produces a bounded gap or
lower supported fidelity, never a fake exact span. Expansion follows the same domain and retained references.

## Preserve failure scope

| Observation | Result meaning |
|---|---|
| Foreign/malformed nomination, absent or contradictory pinned evidence key | Backend/view inconsistency; stop that result/leg rather than silently skip or search another source. |
| A selected item's retained object/representation is genuinely missing or unsupported | Candidate-local gap with the actual reason. It may be consumed only when reported in the delivered page. |
| Corrupt object, conflicting map or invalid range | Corruption gap/failure; emit no invented excerpt and preserve the cause. |
| Cancellation, session revocation, deadline, aggregate request/read/output exhaustion | Stop the request/page at the unhandled reference. Not an item-local absence and not permission to refill. |
| Temporary storage/system failure | Retryable request/backend failure as appropriate; do not flatten it into permanent missing evidence for every candidate. |

Keep these distinctions across the concrete reader and query call, without creating a generic error platform.
Charge work actually performed even on failure. A legal bounded partial response may report earlier handled
items plus a request stop reason; the untouched tail remains unprocessed, not a list of inferred gaps. The
window owner commits only its actually delivered prefix. Observed cancellation does not emit a candidate page.

Formatting preserves final rank and uses the remaining encoded-byte allowance. Verification of an excerpt
proves its source relation, not ranking quality, current worktree state or exhaustive absence. Bounded top-k,
window exhaustion and count-only refill do not establish complete-negative coverage.

## Cutover and evidence

Change actual query/view/reader/result consumers together. Remove caller expected-digest/profile/residency
booleans, grants/permits, per-call budget resets, fallback candidate IDs and count-only completeness/refill.
Inline a small surviving lookup/read beside its real consumer rather than retaining a crate for fixtures.

After implementation/minimal Clippy, check real retained evidence; a foreign composite key; wrong bytes despite
echoed digests; UTF-16 and newline-normalized excerpts; clipped mapped locations; missing artifact versus cancelled
read; transient storage failure; finite aggregate reads and output; and same-view expansion. The shared validator
and selected daemon paths were read, not every feature/caller. No runtime execution is claimed by this audit.
