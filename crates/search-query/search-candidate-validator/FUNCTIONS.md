# search-candidate-validator — current API and removal boundary

## Current API

| Function | Actual behavior |
|---|---|
| `precheck(nomination, context)` | Compares caller request/plan/membership/collection/epoch/security fields and returns proceed/gap/contamination. |
| `build_readback_request` | Copies expected revision/unit/profile/digest/residency fields from the public nomination into another request. |
| `reopen_and_verify` | Calls a port and compares returned identity/digest/profile/residency/assurance/coordinate/length fields. It does not hash `readback.bytes`. |
| `recheck_before_emission` | Compares another supplied live-security structure and permit. |
| `validate` | Composes the above around a per-call candidate ordinal/read ceiling. |
| `material_coverage_change` | Uses before/after counts and caller refill budget to choose unchanged/refill/replan/incomplete. |

`src/lib.rs` is about 24,524 bytes. Public-main production retained readback is not connected; fixed test
ports exercise the pure comparisons.

## Replacement

```text
resolve_evidence(hit, published_view, resolved_scope, retained_reader, request_context)
```

1. Reject a hit outside the selected view/scope or with contradictory stored identity as a query-leg
   inconsistency.
2. Lookup the authoritative `ViewManifestEntry` by stable retrieval point ID.
3. Obtain exact source revision, representation/prepared artifact and range from that entry.
4. Reserve remaining read/output budget and call the real retained reader with the absolute deadline and
   cancellation token.
5. Verify the immutable object under the reader's actual content/address/integrity format; check exact range
   and coordinate mapping.
6. Return one private `VerifiedEvidenceItem` containing the actual excerpt and final rank metadata, or an
   explicit candidate gap.

Select the output page before this work and group reads by revision/representation where useful. Do not
read every nomination and discard most of them in projection.

## Remove

- public `CandidateNomination`, `ValidationContext` and caller expected-digest authority;
- profile/residency/coordinate/assurance booleans echoed through the port;
- request/plan/grant/security permit fields already fixed by the pinned view/session;
- per-call candidate ordinals and fresh read budgets instead of the request ledger;
- candidate identity derived from leg ID or profile fallback;
- count-only refill/completeness logic and validation receipts.

Keep one source/view inconsistency versus candidate-gap distinction and actual range/read error mapping.
Ordinary top-k stays non-exhaustive. If no meaningful package boundary remains, move the function beside
query composition/retained reading and remove the crate.

## Verification

Use a real retained reader and view manifest to check correct excerpt, wrong/foreign point, wrong route or
scope, missing revision, corrupted object, invalid range, lossy/unsupported coordinates, cancellation,
deadline and aggregate read/output limits. Echoing expected digests with wrong bytes must not pass.
