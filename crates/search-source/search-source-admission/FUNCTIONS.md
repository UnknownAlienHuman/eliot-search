# Function contract — `search-source-admission`

**Status:** W2/P03 logical contract; no policy runtime or source-admission evidence is accepted.

This package owns deterministic deny-by-default admission meaning. It performs no filesystem/Git I/O,
registers no source and retains no bytes. Callers supply bounded path/metadata/source-kind/sensitivity
observations gathered by the owning platform/source boundary.

## Global rules

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

### `section_digest(validated) -> Result<Blake3Digest32, AdmissionError>`

Retained v1 SHA-256 over the section profile, size and permission fields. The
legacy `Blake3Digest32` container preserves existing bytes and is not BLAKE3
execution evidence. Publicly writable section text is charged before copying;
an over-limit preimage returns `ADMISSION_OBSERVATION_INVALID`, never a digest.

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

### `validate_observation(input, policy, limits) -> Result<AdmissionObservation, AdmissionError>`

Requires bounded normalized locator class (not unrestricted display path), root/location class,
filesystem metadata, source kind, byte-size observation, generated/vendor/binary/system/sensitivity
signals, detector/profile identities and explicit unavailable fields.

Unavailable load-bearing observation yields review/deny according to the policy; it never defaults to
allow. The operation performs no I/O.

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
outcome per input. Cancellation returns an incomplete batch receipt that cannot authorize any missing or
unprocessed item; it never silently drops denied/review cases.

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
