# Agent contract — search-source-admission

Own only `crates/search-source/search-source-admission/`. Do not edit the root workspace, shared
contracts, architecture or another package. Missing fields use the contract-change process.

The bounded implementation packet is `swarm/assignments/search-source-admission.md`.

## Ownership

- pure canonical `SourceAdmissionPolicy` evaluation
- deny-by-default source/path/metadata/format/sensitivity rules
- deterministic decisions and legacy v1 receipts bound to policy and observation digests
- v2 receipt issuance/verification belongs to the permanent `search-source-admission-receipt` leaf; do not restore a forwarding module or reverse dependency

## Forbidden ownership

- opening files or Git objects
- root, identity or membership mutation
- post-admission authorization or ranking
- unknown-field allow behavior

## Dependencies

Only `search-contracts` and `search-domain`. The evaluator performs no filesystem, clock, database,
process or network I/O.

## Size

Target `src/` ≤ 4,500 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust lines.

## #430 corrective ownership and size boundary

The evaluator owns `EvaluationMeterV2` and its private subtraction helper.
Its owner-internal `pub(super)` constructor accepts four admitted `usize` maxima;
it does not depend on `BatchLimitsV2`. Batch validates its existing limits before
constructing that meter and calling the sole evaluator. Keep the module dependency
one-way: batch → evaluator. Public APIs, field/method charging bodies, probes,
errors, profiles and the 1024-occurrence multiset contract remain unchanged.

Remove only the eleven newly added batch fixture bodies and their local helpers
and fixture-only prose/imports. Do not move those bodies to another source owner
or test directory to hide size. Retain historical evaluator, observation, policy
and v1 fixtures. Preserve the exact deferred case/assertion inventory under
#427/#215 and in [BATCH_V2.md](BATCH_V2.md); removed bodies are absent from this
corrective candidate, not passing tests. Historical authored bodies were not run.

At #430, the complete corrected owner was 8,672 physical handwritten Rust lines TOTAL,
including every retained fixture: batch 308 + evaluator 1,723 + lib 4,442 +
policy 1,303 + observation 896. It removes 858 lines from accepted TOTAL 9,530.
This still exceeds the 8,500 split-review threshold. Independent split review and manager pre-edit acceptance under #430 bind this
exact owner to a new 8,700-line ceiling for this correction only. Final recount
is required; the 10,000 hard stop stays.
The prior batch allowance is obsolete and does not transfer; no new blanket
growth grant, fixture exclusion or receipt/contracts allowance is implied.

Reuse the measured shared producer; no sizing formula, second encode, duplicate
evaluator, forwarding split or partial-success fallback. Original probes and
all-refusal cleanup stay unchanged. Source gates and qualification remain separate.

## #428 fixture-only size boundary

Independent whole-owner review and manager pre-edit acceptance bind this exact
repair to 8,673 TOTAL physical handwritten Rust lines, including every retained
fixture: batch 308 + evaluator 1,724 + lib 4,442 + policy 1,303 + observation 896.
The only Rust delta is three removed/four added lines in the existing evaluator
CBOR golden oracle. Production code, identities, APIs, probes and every assertion
remain unchanged. This fresh allowance applies only to #428; no #430 allowance
transfers and no future growth is granted. The 8,500 review threshold and 10,000
hard stop remain. The corrected fixture is AUTHORED_NOT_RUN under #215.
