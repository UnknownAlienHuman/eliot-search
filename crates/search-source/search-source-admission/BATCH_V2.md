# Admission batch v2 (#427)

Pure complete admission-policy evaluation. Source gates and integrated qualification
remain separate; see #427 for the exact frozen implementation/review status.

## Owning API

Rust path: `search_source_admission::batch_v2`.

```rust
pub fn evaluate_batch_v2(
    policy: &CompiledAdmissionPolicyV2,
    observations: &[AdmissionObservationV2],
    scratch: &mut MatchBufferV2,
    limits: BatchLimitsV2,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<AdmissionBatchV2, AdmissionError>;
```

The same accepted policy, observation, decision, match-buffer, error and checkpoint
types retain their actual owners. One private evaluator worker produces all
decisions; no second evaluator, per-item receipt or v1 facade exists.
The result has private schema, policy revision/fingerprint, decision Vec and batch
fingerprint fields. Readonly getters are `schema_version()`, `policy_revision()`,
`policy_fingerprint()`, `decisions() -> &[AdmissionDecisionV2]` and
`fingerprint() -> BatchFingerprintV2`; fingerprint bytes are readonly.
There is no caller-supplied digest, accepted flag, constructor, codec or mutable
view. Clone retains the same immutable provenance. Debug contains schema/count
and a redacted fingerprint, without decisions, rule IDs, metadata or locator bytes.

## Occurrences, counts and complete-byte work

Every occurrence is evaluated and charged, including duplicate observations.
Processing/refusal order follows the caller slice; sorting occurs only after
every decision succeeds. The moved decisions are sorted by observation-fingerprint
bytes then decision-fingerprint bytes. No deduplication occurs. Output order is
canonical multiset order, not input-index order. Deny, ReviewRequired and Unsupported
remain their existing decisions; a complete batch is not a grant of Allow.

| Raw BatchLimitsV2 field | Inclusive maximum |
| --- | ---: |
| max_items | 1024 |
| max_rule_evaluations | 131072 |
| max_matched_rule_ids | 131072 |
| max_reason_contributions | 23552 |
| max_encoded_bytes | 8388608 |
| per_item | Existing EvaluationLimitsV2 five-field validation |

All raw dimensions must be positive and within their fixed maxima. `per_item`
retains existing rule128, ID128, reason23, decision65536-byte and
observation16384-byte maxima and accepted lower ceilings. Validation happens once
before reserve/work; empty input or input longer than max_items returns
ObservationInvalid. Defaults use the fixed aggregate and accepted per-item maxima.

A fresh private meter charges the complete canonical rule count before each item's
observation/matcher work; each matching ID before clone; and each NEW reason before
insertion. Baseline, losing outcome categories and DenyByDefault all count.
Duplicate reasons within one item cost once; separate occurrences cost again.
Returned winning reasons retain the accepted evaluator's filtering semantics.

For successful calls, encoded work is the sum of each performed complete
observation revalidation, each decision computation and the final batch computation.
Each includes its own domain + NUL + containing CBOR and rows. The sole accepted
shared counted stream produces both fingerprint and exact successful usage in
one pass. Each frame receives min(frame ceiling, remaining aggregate); actual
successful bytes are subtracted. Derived zero refuses as BudgetExhausted.
A failing frame ends the whole call; no partial usage/result is exposed.

These are count/serialized-work bounds, not heap, wall-clock or resource qualification.
Caller model/policy construction, bounded row allocation before sink charging and
separately invoked receipt operations are not covered by a new heap/whole-program claim.
No sizing formula, cached encoded length, worst-case reservation, second encode or
complete preimage buffer is used.

## Refusal and cleanup

Clear match results on outer entry and every error, preserving the scratch's
policy fingerprint fence. Entry checkpoint precedes raw validation; later existing
evaluator/matcher refusals retain their ordering with added cumulative guards.
The original probe is reused through evaluation, sorting/hash boundaries and after
result construction. No clock, renewed deadline or hard-preemption claim is added.

Invalid raw dimensions/count return ObservationInvalid; actual count/byte exhaustion
returns BudgetExhausted. Checkpoint failures retain Cancelled/DeadlineExceeded mapping.
Only shared OversizePayload at digest_preimage/canonical_sink becomes BudgetExhausted;
other shared producer errors become ObservationInvalid. On any refusal, the local
decisions are discarded and no AdmissionBatchV2 or incomplete success is returned.

## Equality profile and private seams

Schema2; profile `eliot-admission-batch-multiset@2`;
domain `eliot/cbor/admission/batch/v2`. Shared top-array rows4+N:

0. `[2, profile, 1024, 131072, 131072, 23552, 8388608]`;
1. policy revision;
2. policy fingerprint bytes32;
3. occurrence count;
4. onward: one sorted `[observation fingerprint bytes32, decision fingerprint bytes32]`
   for EVERY occurrence, including duplicates.

Sufficient lower operation limits and private usage counters are excluded from
identity. Multiplicity affects identity; successful permutations produce the same
multiset identity. Existing v1, policy, observation, decision and receipt profiles,
rows, bytes and public signatures remain BYTE_COMPATIBLE.
The new batch profile's future adoption is VERSIONED_REBUILD under270/271.

Owner-internal `evaluate_v2_metered(..., limits, Option<&mut EvaluationMeterV2>, probe)`
is pub(super). Public evaluate_v2 delegates with None and retains raw validation;
the sole batch caller validates limits before passing Some. In the #430 correction, `evaluation_v2` owns the private meter and subtraction helper. Its
`pub(super) fn new(rule_evaluations: usize, matched_rule_ids: usize,
reason_contributions: usize, encoded_bytes: usize) -> Self` receives exactly the
four already-admitted batch maxima. Fields stay private; charging methods remain
pub(super) with unchanged bodies. The evaluator imports no batch type; batch
imports the meter from the evaluator, keeping composition one-way.
Observation `validate_encoded_limit_with_usage(&self, usize)` is pub(super), returns
the actual shared CanonicalBlake3Output and retains fingerprint comparison.
Public validate_encoded_limit still returns Result<(), AdmissionError>.
No shared usage output is an authority or generic operation receipt.

## Scope, size and evidence

Frozen #430 owning paths: src/batch_v2.rs, src/evaluation_v2.rs,
AGENTS.md, README.md, FUNCTIONS.md, BATCH_V2.md and the exact canonical ledger.
The meter/helper moves and batch constructor binding are the production change;
only the eleven new batch fixture bodies/local helpers and fixture-only prose/imports
are deleted. No lib/observation/policy/root manifest/lock/dependency/contracts/
receipt-leaf/daemon/ingestion change. Receipt ownership stays in its permanent leaf.
Pure evaluation gives no native containment, root/source identity, access or currentness proof.

The complete corrected owner measures TOTAL 8,672 physical handwritten Rust lines:
batch 308 + evaluator 1,723 + lib 4,442 + policy 1,303 + observation 896,
including every retained fixture. This is 858 fewer than accepted TOTAL 9,530.
It remains above the 8,500 split-review threshold and below hard 10,000.
Independent split review and manager pre-edit acceptance under #430 bind this
exact owner to a new 8,700-line ceiling for this correction only; final recount
is required. The obsolete
#427 size addendum is removed; no prior allowance or future growth grant transfers.
There is no forwarding crate, hidden fixture move, duplicate evaluator, reduced
capacity or changed profile. #430 records the exact ledger closure and pre-edit Ready/freeze.

The eleven batch cases below are deferred and their Rust bodies are absent from
this corrective candidate. Their prior authored, unexecuted bodies remain in Git
at `21c073e993bde903b35e1cf2a9de5575dfd68bc7`. Historical evaluator/observation/
policy/v1 fixtures stay unchanged and AUTHORED_NOT_RUN. This inventory retains
requirements for later integrated implementation/execution under #427/#215;
it is neither present tests nor a passing oracle.

Corrective production/static gate results are recorded in #430 and its PR at
the final corrected SHA; earlier #427 results are not new-head evidence.
Daemon D387 remains NOT_PASS/unrun. Source presence/compilation is not native,
runtime, live-Qdrant, installed, resource, scale or product qualification.
Known retained evaluator golden F1/#428 remains unrepaired and is not a passing
oracle; its correction/execution and F1/#426 receipt documentation stay separate.
#241/#405, Wave2/#235 and Goal remain incomplete.

## Deferred exact batch cases (#427/#215)

Names and expectations below retain the historical eleven cases. Numeric values
are their authored boundary assertions, not observed execution or resource results.

1. `capacity_is_1024_occurrences_and_refusal_has_no_success_value`: 1,024 identical
   no-rule occurrences yield 1,024 Deny decisions at 450,688 complete bytes;
   450,687 gives BudgetExhausted. Empty/1,025 inputs are ObservationInvalid;
   max_items=1 admits one and refuses two.
2. `permutation_is_a_multiset_with_duplicate_occurrences_retained`: [a,b,a] and
   [b,a,a] have equal fingerprints/decisions; removing one a changes identity.
   Three outputs match separately evaluated decisions sorted by observation then
   decision fingerprints, with nondecreasing pair order.
3. `exact_actual_bytes_include_both_observations_decisions_and_final_preimage`:
   no-rule item ceilings 194 observation/177 decision; two items cost
   2*(194+177)+262=1,004. Independently framed final CBOR is 232 bytes, complete
   domain-plus-NUL frame 262, and parsed canonical hashing equals batch identity.
   Generous/exact limits have equal identity/results; aggregate caps
   [1,193,194,370,371,564,741,742,1003] refuse BudgetExhausted, including zero
   remaining between frames. Per-item 193/176 refuse; one item fits 564, not 563.
4. `rule_id_and_losing_reason_charges_precede_winning_category_filter`: two
   permit+block items require four rules, four IDs and four distinct reason
   contributions; each returns DenyExplicitRule while retaining both IDs.
   Reducing any aggregate to three refuses and clears scratch; per-item rules,
   IDs or reasons limited to one also refuse. Losing Allow contributions count.
5. `unmatched_rules_still_charge_work_and_same_reason_charges_once_per_item`:
   two items with one hit/one miss charge four rules, two IDs and two reasons;
   three rules refuse. Two matching Allow rules per item retain both IDs but
   charge one AllowExplicitRule reason per item; two contributions fit, one refuses.
6. `deny_by_default_is_charged_for_every_occurrence_without_implicit_allow`:
   two no-rule items return Deny/DenyByDefault and need two reason contributions;
   one refuses BudgetExhausted, with no implicit Allow.
7. `later_item_refusal_preserves_input_processing_order_and_returns_no_partial_batch`:
   a good item alone fits 564; good then missing locator refuses
   ObservationIncomplete under generous limits. At aggregate 371, good then
   missing refuses BudgetExhausted; missing then good refuses ObservationIncomplete.
   Every refusal returns an error, not a partial batch.
8. `all_raw_dimensions_validate_and_entry_checkpoint_wins_with_cleanup`:
   all five aggregate and five per-item raw dimensions refuse zero/maximum+1,
   including unused no-rule ceilings and empty input. Seeded match results clear.
   Entry Cancelled/DeadlineExceeded takes precedence over invalid raw/empty input,
   maps to Cancelled/BudgetExhausted and clears scratch.
9. `every_original_checkpoint_including_final_refuses_cleans_and_allows_reuse`:
   count the healthy two-item probe calls; inject each closed checkpoint failure
   at every call including final return. Stop at that call, return the mapped
   error, clear seeded matches, and allow same-scratch retry with the healthy
   fingerprint and decisions. This specifies cooperative checkpoints, not preemption.
10. `later_missing_locator_and_final_byte_refusal_clear_previously_populated_scratch`:
    later missing locator returns ObservationIncomplete and clears seeded matches.
    Single-ID Allow needs 194 observation+188 decision+193 final=575 bytes;
    574 refuses at the final frame and clears; 575 permits retry with Allow.
11. `all_outcomes_survive_and_debug_and_errors_exclude_payload_markers`:
    ordinary/system/missing-detector/device inputs separately yield
    Allow/Deny/ReviewRequired/Unsupported; batch retains their sorted decisions.
    Normal/alternate Debug of batch and missing-locator error exclude detector
    marker BATCH_PRIVATE_DETECTOR_91, path marker BATCH_PRIVATE_PATH_91,
    rule ID batch-private-rule-91, private/* pattern and BASELINE_PROFILE text.

## Corrected #427 chronology

Saved issue edit history (`userContentEdits(first:100)`, hasNextPage=false) and
review records establish the publication/submission times below. Commit times
come from exact Git objects; the merge event time is GitHub's merged_at field.
The complete reconciliation is public in #430 comment 6108431996.
All times are UTC on 2026-10-11.

| Event | Time |
| --- | --- |
| #427 body edit to READY/SCOPE_FROZEN | 10:54:23 |
| First source commit ecbebb74 | 10:54:58 |
| Second source commit 54d00158 | 10:55:41 |
| Final source commit 21c073e9 | 10:57:46 |
| Formal manager COMMENT review 5483058046 | 11:05:46 |
| PR429 merge event | 11:06:24 |
| Later review 5483066438, COMMENTED state with blocking text | 11:07:55 |

The published freeze preceded the first commit by 35 seconds; the later blocking
text was submitted 91 seconds after the recorded merge. These records do not
support a post-edit freeze or merge-after-that-review claim. The corrective
layering/fixture scope remains valid independently of those chronology claims.
Monitoring remains ACTIVE by explicit user direction; its status is not a gate,
review execution or qualification result. This correction changes no scheduler.
