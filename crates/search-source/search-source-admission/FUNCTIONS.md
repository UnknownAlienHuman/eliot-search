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

### `section_digest(validated) -> Blake3Digest32`

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

Domain-separated BLAKE3 over canonical policy schema/revision/behavior bytes. Any load-bearing change
produces a new fingerprint.

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
