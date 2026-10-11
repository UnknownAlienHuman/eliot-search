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
the sole batch caller validates limits before passing Some. Meter creation/fields
are private; its charging methods are pub(super).
Observation `validate_encoded_limit_with_usage(&self, usize)` is pub(super), returns
the actual shared CanonicalBlake3Output and retains fingerprint comparison.
Public validate_encoded_limit still returns Result<(), AdmissionError>.
No shared usage output is an authority or generic operation receipt.

## Scope, size and evidence

Owning files: batch_v2.rs; minimal lib module declaration; evaluator meter
hooks; observation private usage seam; owning README/FUNCTIONS/BATCH_V2 and size
addendum; exact canonical ledger. No root manifest/lock/dependency/policy/contracts/
receipt-leaf/daemon/ingestion change. Receipt ownership stays in its permanent leaf.
Pure evaluation gives no native containment, root/source identity, access or currentness proof.

Fresh projected TOTAL9533 =8198 +1232 batch +78 evaluator +22 observation +3 declaration,
including eleven new private causal tests and retained fixture code. This crosses8500
split review and remains below hard10000; explicit independent pre-edit whole-owner
acceptance is required. No receipt/contracts exception or future blanket growth grant transfers.
The exact full glue/docs/ledger, size and production/static gates are bound in #427 Ready/freeze. The slice-specific reviewed whole-owner cap is9600; hard10000 remains.
Gate results belong to the exact reviewed source revision; this document alone proves none. All fixtures are AUTHORED_NOT_RUN Q215;
daemon D387 remains NOT_PASS/unrun. Source presence or future compilation is not
runtime/native/live-Qdrant/installed/resource/scale/product qualification.


Known retained evaluator golden F1/#428 is not a passing oracle. Its minimal canonical-hash correction and integrated execution remain separate.
