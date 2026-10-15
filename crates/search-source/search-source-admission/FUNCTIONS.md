# Function contract — `search-source-admission`

**Status:** W2/P03 logical contract; no policy runtime or source-admission evidence is accepted.

This package owns deterministic deny-by-default admission meaning. It performs no filesystem/Git I/O,
registers no source and retains no bytes. Callers supply bounded path/metadata/source-kind/sensitivity
observations gathered by the owning platform/source boundary.

## Global rules

`original_envelope` retains bounded original v1 policy/observation/receipt input
for exact historical restoration through its owning evaluator and codec.
Restoration proves that original input, not current policy, native read access
or serving. It is not a fallback from the permanent v2 admission surface.

The permanent v2 surface is `evaluation_v2::evaluate_v2(policy, observation, scratch, limits, probe)`
returning immutable `AdmissionDecisionV2`. See [EVALUATION_V2.md](EVALUATION_V2.md) for exact finite
ceilings, deny/unsupported/review/explicit-Allow resolution, whole-call locator refusal, error cleanup
and eight-row decision identity. V2 has no implicit baseline Allow or v1 authority fallback. Its pure
decision does not grant native/source/access/currentness authority or supply a verified receipt.

- every decision binds one canonical policy revision/fingerprint and one canonical observation digest;
- unknown load-bearing fields, source classes or rule operators fail closed;
- unconditional safety denies cannot be bypassed by an unspecified/implicit override;
- equal canonical inputs produce byte-identical ordered reasons, decision and receipt;
- decisions authorize only source admission under the named policy, not later access or currentness;
- reusable fixtures and default diagnostics contain no user-specific absolute path or source body.

## Configuration operations

### `section_descriptor() -> ConfigSectionDescriptor`

### `compiled_defaults() -> ConfigSectionInput`

### `validate_section(input, platform, accepted_capabilities) -> Result<ValidatedAdmissionConfig, AdmissionError>`

Implements `config/sections/source_admission.md`. Baseline keeps generated/vendor/binary and sensitive
classes denied unless an explicit accepted policy profile permits them. Known credential/private-key
classes remain unconditional deny.

`validate_section` is the sole public producer of `ValidatedAdmissionConfig`.
The five fields are private and expose only `bootstrap_policy_profile()`,
`max_file_bytes()`, `allow_generated()`, `allow_vendor()` and `allow_binary()`.
There is no public raw-field constructor, setter or mutable getter. Callers
edit `ConfigSectionInput` and validate a new value; later raw-input mutation
cannot modify a retained section. This is schema/data validation, not an
applied-effect or source-admission receipt.

### `section_digest(validated) -> Result<Blake3Digest32, AdmissionError>`

Retained v1 SHA-256 over the section profile, size and permission fields. The
legacy `Blake3Digest32` container preserves existing bytes and is not BLAKE3
execution evidence. Constructor-only section values retain the same v1 field
order and bytes. The existing writer still charges each field before copying;
an over-limit synthetic owner-private preimage returns
`ADMISSION_OBSERVATION_INVALID`, never a digest. Its fallible signature stays.

The #406 fields-to-methods Rust API hardening is deliberate; stored identities
remain BYTE_COMPATIBLE. The validator, retained writer and reload bodies are
unchanged. Four private causal fixture groups are AUTHORED_NOT_RUN under
#352/#241/#215; the existing private oversized section fixture is codec-defense
evidence only. No consumer cutover, effective v2 policy or lower-budget claim.

### `plan_section_change(old, new) -> Result<SectionReloadDecision, AdmissionError>`

Every policy-affecting change creates a new admission policy revision and preserves `SECURITY_BARRIER`.
Permissive changes do not mutate existing membership automatically; restrictive changes require
invalidation/reconciliation obligations from their owners.

## Policy operations

### `validate_policy_schema(input) -> Result<AdmissionPolicyDraft, AdmissionError>`

Checks schema version, closed rule/operator/reason sets, explicit precedence, unconditional deny classes,
finite list/text/pattern/size bounds and override authorization. Duplicate/conflicting rules fail.

### `normalize_policy(draft) -> Result<CanonicalAdmissionPolicy, AdmissionError>`

Canonicalizes path/source classes, ordered rules, pattern encodings, thresholds and override scopes.
Serialization is deterministic and independent of map/input order.

### `policy_fingerprint(policy) -> PolicyFingerprint`

Retained domain-separated v1 SHA-256 over policy schema/revision/behavior bytes. Any load-bearing change
produces a new fingerprint.

### Retained v1 digest execution boundary (#320)

One existing package-local writer supplies exact bytes to the shared bounded
`sha256_exact_bytes` API. It adds no shared CBOR or raw-domain framing. Copied SHA
constants, padding and compression are deleted. The complete preimage ceiling is
`MAX_DIGEST_PREIMAGE_BYTES` (1,048,576); refusal is sticky and precedes each buffer
extension and algorithm execution. Receipt/batch errors propagate through their
existing `Result` boundaries. Opaque policy and observation values are bounded by
their only public constructors; their signatures remain infallible.

| Domain (prefix `eliot-search/`) | Retained field order after domain + NUL |
|---|---|
| `source-admission-section/v1` | profile, max-file bytes, generated/vendor/binary flags |
| `admission-policy/v1` | schema, revision, max-file bytes, flags, rule count; each rule: ID, precedence, class, operator, effect, reason, optional size/sensitivity/locator/pattern |
| `admission-observation/v1` | locator/location/kind/class, optional size and generated/vendor/binary/system flags, optional sensitivity/detector/profile, unavailable count and sorted fields |
| `admission-receipt/v1` | schema, revision, fingerprint, observation digest, outcome, reason count and ordered reasons, matched-ID count and ordered IDs, sensitivity |
| `admission-batch/v1` | revision, fingerprint, item count; each ordered item: observation digest, outcome, ordered reasons |

Integers remain fixed big-endian (u32 schema, otherwise u64); bytes/text use a u64
length and optional values a one-byte 0/1 tag. Policy schema and receipt schema
remain 1. All five bounded identities are BYTE_COMPATIBLE; no profile migration or
stored-byte restoration is algorithm execution proof. Independent-field upper
bounds are policy 188,352, observation 860, receipt 34,639 and batch 738,392 bytes.
Validated default section is 76 bytes; arbitrary public section text is fallible.
Lower caller limits and their later v2 cutover are not changed by this absolute
ceiling. No new digest type, second codec or dependency is introduced.

Four private fixtures cover five independent frozen goldens, domain/framing
distinction, constructor bounds and exact-ceiling/overflow refusal. They are
AUTHORED_NOT_RUN under #352/#215; source gates establish no product qualification.

### `classify_policy_change(old, new) -> AdmissionPolicyChange`

Returns exact changed rule/classes plus:

```text
NOOP
RESTRICTIVE_SECURITY_BARRIER
PERMISSIVE_RECONCILE_REQUIRED
MIXED_SECURITY_BARRIER_AND_RECONCILE
REJECT
```

It does not apply the change or decide which registered sources survive.

## Observation operations

### Pure v2 observation model (#409)

`observation_v2::AdmissionRelativeLocator::from_bytes(&[u8])` validates borrowed
UTF-8 bytes before retaining them. Fixed profile
`eliot-admission-relative-utf8-portable@1` preserves exact bytes and rejects
malformed, traversal, absolute and nonportable native spellings. Limits are
4,096 bytes total, 255 bytes per component and 128 components. It proves a byte
vocabulary only; containment and native identity remain with their owners.

`AdmissionObservationV2::new(AdmissionObservation, Option<AdmissionRelativeLocator>)`
owns existing constructor-bounded metadata and explicit locator availability.
It computes opaque `ObservationFingerprintV2` through shared
`blake3_canonical_array`, with the fixed fifteen-row schema and complete
16,384-byte domain-plus-CBOR ceiling specified in [OBSERVATION_V2.md](OBSERVATION_V2.md).
Absent optionals encode as canonical Null. Shared failures return the existing
content-free `ADMISSION_OBSERVATION_INVALID` and no model or partial identity.

Only read-only getters are exposed. Debug omits metadata IDs and locator bytes.
No constructor accepts a supplied digest, acceptance flag or profile. Equality
grants no match, admission, receipt, access or currentness authority. All v1
types, APIs, consumers and preimages are unchanged; future adoption requires
VERSIONED_REBUILD, with no migration in this model-only slice. Six private
causal fixture groups and an independent synthetic golden are AUTHORED_NOT_RUN;
execution remains under #352/#241/#215.

### Caller-selected v2 observation ceilings (#414)

`AdmissionObservationV2::new_with_encoded_limit(metadata, relative_locator, max_encoded_bytes)`
and `validate_encoded_limit(&self, max_encoded_bytes)` share one private complete-fingerprint
producer with the existing `new`. The retained-model check recomputes and compares its private
fingerprint without mutation. Zero or greater than 16,384 is ObservationInvalid before row work.
Accepted ceilings are passed directly to the shared complete domain + NUL + CBOR charge. Only
OversizePayload at shared `digest_preimage`/`canonical_sink` maps to BudgetExhausted in these
new APIs; every other shared error maps to ObservationInvalid. Legacy `new` retains all-error
ObservationInvalid behavior. Errors carry no input text and no partial identity is returned.

The operation ceiling does not change the fixed profile/header/rows or retained identity bytes.
No budget flag, cached length, supplied digest or second model is added. Already bounded row
values may be copied before the serialized-byte charge. Original caller checkpoints belong to
future evaluator/batch composition; these pure synchronous data-model calls add none. Private
boundary/parity/readonly/error cases are AUTHORED_NOT_RUN, Q/#215. Other lower/raw limits remain
#405 and effective evaluator/receipt/batch remain #241.

### `validate_observation(input, policy, limits) -> Result<AdmissionObservation, AdmissionError>`

Requires bounded normalized locator class (not unrestricted display path), root/location class,
filesystem metadata, source kind, byte-size observation, generated/vendor/binary/system/sensitivity
signals, detector/profile identities and explicit unavailable fields.

Unavailable load-bearing observation yields review/deny according to the policy; it never defaults to
allow. The operation performs no I/O.

The raw unavailable-marker list is capped at the eight closed names before its first traversal or
clone. A longer list returns `ObservationInvalid`, including when it contains an unknown token.
Earlier limits, unknown fields, policy-rule count and class/kind parses retain their precedence.
Within the cap, unknown, duplicate, missing and contradictory marker behavior stays exact. Every
previously accepted observation and its v1/v2 identity bytes remain **BYTE_COMPATIBLE**. This bounds
the validator's list work; it makes no claim about allocations already performed by its caller.

### `observation_digest(observation) -> AdmissionObservationDigest`

Hashes canonical content-free observation fields and classifier/profile identities. It does not hash
source body because baseline admission owns no body classifier.

## Decision and receipt operations

### Legacy v1 pattern authority boundary (#404)

One private guard refuses any rule with `Some(pattern)` at `evaluate`,
`evaluate_batch`, `issue_receipt` and `verify_receipt`, using the existing closed
`ADMISSION_POLICY_SCHEMA_UNSUPPORTED` error. V1 retains patterns for diagnostic
validation, normalization and fingerprints; it implements no path predicate.
Pattern-bearing v1 authority is LEGACY_DECODE_ONLY, with no new successful
decision, batch, receipt or verified receipt. Batch refusal is whole, without
partial results. Policies without patterns and all retained writers remain
unchanged; the effective v2 matcher/relative-locator owner belongs to #241.

Evaluation preserves cancellation and zero-budget precedence; batch also retains
limits/count precedence. Issuance refuses before identities/copies. Verification
checks receipt schema first, then refuses before identities/copies. Five private
refusal/parity/precedence fixtures are AUTHORED_NOT_RUN under #352/#241/#215;
historical receipt helpers are `cfg(test)` only and create no production fallback
or public proof constructor. No public signature or persisted-byte delta.

### `evaluate(policy, observation, budget, cancel) -> Result<AdmissionDecision, AdmissionError>`

Applies the closed rule order and returns:

```text
ALLOW
DENY
REVIEW_REQUIRED
UNSUPPORTED
```

with stable ordered reason codes, matched rule IDs, maximum disclosure/sensitivity class and bounded
non-content explanation metadata.

Cancellation/budget exhaustion returns no successful decision. No partial rule evaluation may be
advertised as allow.

### `issue_receipt(policy, observation, decision) -> Result<AdmissionReceipt, AdmissionError>`

Requires decision inputs equal the supplied canonical policy/observation identities and produces an
immutable receipt binding policy revision/fingerprint, observation digest, decision/reasons, sensitivity
class and schema version.

### `verify_receipt(receipt, current_policy, observation) -> Result<VerifiedAdmissionReceipt, AdmissionError>`

Recomputes every identity and rejects stale policy revision/fingerprint, mismatched observation,
unsupported schema, altered reasons or a decision inconsistent with current unconditional denies.

A valid old permissive receipt does not bypass a newer restrictive policy. Registry/currentness owners
must use a receipt accepted for their exact policy fence.

### `redacted_decision_view(decision, disclosure) -> AdmissionDecisionView`

Returns decision/reasons/rule IDs and location/source classes only. Absolute paths, classifier secret
features and source bytes are excluded.

## Batch operations

### `evaluate_batch(policy, observations, limits, budget, cancel) -> Result<AdmissionBatch, AdmissionError>`

Validates finite count/encoded bytes, canonicalizes by observation identity and returns one explicit
outcome per input. Cancellation or budget exhaustion returns an error without a successful batch
or partial receipt. A successful batch never silently drops denied/review cases.

## Cancellation, deadline and retry

All admission operations are pure CPU work with finite budgets and cancellation checkpoints. Repeating
equal inputs is safe. There is no durable mutation or unknown commit outcome. A cancelled/error result
cannot be converted to allow by a caller fallback.

## Typed failures

- `ADMISSION_POLICY_SCHEMA_UNSUPPORTED`
- `ADMISSION_POLICY_UNKNOWN_FIELD`
- `ADMISSION_POLICY_CONFLICT`
- `ADMISSION_POLICY_OVERRIDE_FORBIDDEN`
- `ADMISSION_OBSERVATION_INVALID`
- `ADMISSION_OBSERVATION_INCOMPLETE`
- `SOURCE_ADMISSION_DENIED`
- `SOURCE_ADMISSION_REVIEW_REQUIRED`
- `SOURCE_KIND_UNSUPPORTED`
- `SOURCE_TOO_LARGE`
- `SENSITIVE_SOURCE_DENIED`
- `ADMISSION_RECEIPT_MISMATCH`
- `ADMISSION_RECEIPT_STALE`
- `ADMISSION_BUDGET_EXHAUSTED`
- `ADMISSION_CANCELLED`

## Required tests / qualification evidence

- canonical policy/observation/decision/receipt goldens and input-order independence;
- default credential/private-key/system/cache/build/vendor/generated/binary deny fixtures;
- unknown field/operator/source class fails closed;
- unconditional deny cannot be overridden unless exact schema explicitly permits it;
- missing detector/metadata signal never silently allows;
- policy revision/fingerprint change invalidates receipt;
- restrictive/permissive/mixed change classification;
- batch accounts one outcome per input and cancellation cannot authorize incomplete items;
- no I/O, registry, filesystem/Git/source-body dependency;
- path/source-body/debug/serialization disclosure audit;
- `source_admission` config defaults, bounds and security-barrier fixtures;
- property tests proving equal canonical input yields byte-identical output.
## Compiled v2 path classification (#412)

The additive [`policy_v2`](POLICY_V2.md) module compiles an immutable bounded
policy with one reviewed `globset` matcher, an owner-computed shared
CBOR/BLAKE3 fingerprint and reusable policy-bound match storage.
`compile_policy_v2` accepts raw rules and validated section data;
`match_path_rules` selects canonical rule indexes from accepted relative bytes.
It supplies no admission decision, receipt, batch or native authority.

Both calls require a borrowed caller checkpoint for the original absolute
deadline and live cancellation. Closed `Cancelled`/`DeadlineExceeded` failures
map to existing cancelled/budget-exhausted errors. No owner clock, context,
lock or hard preemption is introduced. Match failures clear all results.

V1 bodies and preimages remain **BYTE_COMPATIBLE**. Future v2 adoption requires
**VERSIONED_REBUILD**; this slice performs no consumer cutover or v2-to-v1
facade. Effective evaluation is supplied by [evaluation_v2](EVALUATION_V2.md);
receipts/batch remain #241/#405. Private causal
fixtures are **AUTHORED_NOT_RUN** under #352/#241/#215. Donor provenance and
source gates do not qualify native, runtime, scale or installed behavior.

## Permanent v2 receipt owner (#424)

The complete v2 receipt issuer/verifier and its private provenance values now
belong to [search-source-admission-receipt](../search-source-admission-receipt/README.md).
Callers use `search_source_admission_receipt::receipt_v2`; the old module is deleted,
without a forwarding reexport or duplicate implementation. Policy, observations
and effective decisions stay in this package. Receipt schema/domain/profile/bytes
and all existing v1 behavior remain unchanged. Native/root/source/access/currentness
adoption and qualification stay separate under #270/#271/#215.

This removes the whole receipt responsibility and its causal fixtures from this
owner. Receipt-only9200 permission is historical and grants no new batch growth;
The complete batch is implemented under #427; #430 records its reduced whole-owner review.

## Cumulative multiset v2 batch (#427)

The additive owning contract is described in
[BATCH_V2.md](BATCH_V2.md); its exact Ready/freeze and source review are tracked by #427.

`batch_v2::evaluate_batch_v2(policy, observations, scratch, limits, probe)`
returns `Result<AdmissionBatchV2, AdmissionError>`. Its result is private,
immutable and complete, with readonly schema/policy/decisions/fingerprint getters.
It evaluates every occurrence, including duplicates, through the existing
effective evaluator. Successful decisions are sorted by observation fingerprint
then decision fingerprint, retaining multiplicity and Deny/Review/Unsupported.
No incomplete success, receipt, public codec, accepted flag or supplied digest
is added. Empty or over-limit input refuses; capacity remains 1024 occurrences.

`BatchLimitsV2` admits positive finite `max_items` through1024,
`max_rule_evaluations` and `max_matched_rule_ids` through131072,
`max_reason_contributions` through23552, `max_encoded_bytes` through8388608,
and the existing `EvaluationLimitsV2` in `per_item`.
Raw dimensions validate once before reserve/work. Canonical rules charge before
matcher work; matching IDs charge before clone; each new per-item reason charges
before insertion, including baseline, losing categories and DenyByDefault.
Repeated reasons charge once within an item and again in another occurrence.

One private meter charges actual complete observation revalidation, decision
and final batch preimages using the accepted shared counted CBOR/BLAKE3 stream.
No worst-case reservation, second encode, preimage buffer or local size formula
is introduced. Derived zero remaining bytes is BudgetExhausted. Counts and
serialized work are not a heap/resource bound; caller-side model/policy creation
and separately issued receipts are outside this call's accounting.

The original caller checkpoint wins at entry before raw validation, is reused
through work and is checked after final result construction. Every error clears
match results while preserving their policy fence and returns no batch.
Invalid raw limits/count return ObservationInvalid; actual exhaustion returns
BudgetExhausted. Existing evaluator refusals propagate. Only shared
OversizePayload at digest_preimage/canonical_sink maps to BudgetExhausted;
other shared producer failures map to ObservationInvalid.

Schema2/profile `eliot-admission-batch-multiset@2`, domain
`eliot/cbor/admission/batch/v2`: header with fixed aggregate maxima, policy
revision/fingerprint, occurrence count and one sorted observation/decision
fingerprint pair per occurrence. Sufficient lower limits do not alter equality.
V1 and existing policy/observation/decision/receipt identities remain
BYTE_COMPATIBLE; future batch adoption requires VERSIONED_REBUILD under270/271.

The private evaluator/observation usage seams remain owner-internal;
existing public batch, single-evaluation and observation signatures remain unchanged.
The #430 correction moves `EvaluationMeterV2` and its subtraction helper
into `evaluation_v2`. Its `pub(super)` constructor accepts four admitted `usize`
maxima: rule evaluations, matched IDs, reason contributions and encoded bytes.
Batch constructs it only after existing raw validation and calls the unchanged
`evaluate_v2_metered` worker; public `evaluate_v2` continues to pass `None`.
The evaluator has no batch type/import dependency. Meter fields, charging bodies,
original probe/count/byte/error behavior and all identity profiles stay exact.

No native/root/source/access/currentness authority, receipt-leaf change,
dependency adoption or daemon/ingestion cutover belongs here. At #430, the
corrective whole owner was 8,672 physical handwritten Rust lines including all
retained fixtures (858 fewer than accepted 9,530). This still crosses the 8,500
split-review threshold. #430 records independent review and manager pre-edit
acceptance of a new 8,700-line ceiling for this correction only, with final recount
required. Hard 10,000 stays; no earlier size allowance transfers.

Only the eleven new batch fixture bodies/local scaffolding are removed. Exact
future names/assertions are inventoried in BATCH_V2.md for #427/#215, without
moving Rust bodies elsewhere. These cases are deferred and absent in this
candidate; their historical authored bodies at `21c073e993bde903b35e1cf2a9de5575dfd68bc7`
were not run. Retained evaluator/observation/policy/v1 fixtures are AUTHORED_NOT_RUN.
#428 corrects only the retained evaluator oracle to parsed canonical CBOR/BLAKE3
after its unchanged golden check; the case remains AUTHORED_NOT_RUN under #215.
Receipt-doc F1/#426 remains separate.
Corrective production/static gate results are recorded at the exact head in #430 and its PR; they are not inherited from #427.
Daemon D387 remains NOT_PASS/unrun; Q215 integrated execution and native/runtime/
resource/live-Qdrant/installed/scale qualification are incomplete. Source docs
and source acceptance do not close #241/#405, Wave2/#235 or Goal.
