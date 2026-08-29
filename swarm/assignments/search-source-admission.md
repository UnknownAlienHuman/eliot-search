# `search-source-admission` implementation packet

**Path:** `crates/search-source/search-source-admission`  
**Capability:** C03/C06 security support  
**Delivery:** W2 / P03  
**Gate:** BLOCKED until W0 contracts/domain handoffs are accepted  
**Trace:** S7.4, S16.5-S16.6, H3.5, P03  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust spelling.

## Mission

Produce one deterministic, versioned and auditable admission decision before source preparation.

## Owns

- canonical policy normalization and fingerprinting
- deny-by-default evaluation of path class, metadata, source kind, size and sensitivity observations
- ordered allow/deny/review reasons
- decision receipts bound to policy fingerprint and observation digest

## Must not own

- opening files, following links, reading Git objects or retaining bytes
- registering roots, identities or memberships
- access authorization after admission
- source-body classification in the baseline
- embedding user-specific absolute paths in reusable fixtures

## Logical primitives

- `CanonicalAdmissionPolicy`, `AdmissionObservation`, `AdmissionDecision`, `AdmissionReason`, `AdmissionReceipt`, `PolicyFingerprint`

## Logical operations

1. `normalize_policy(policy) -> Result<CanonicalAdmissionPolicy, AdmissionError>`
2. `evaluate(policy, observation) -> AdmissionDecision`
3. `issue_receipt(policy, observation, decision) -> AdmissionReceipt`
4. `verify_receipt(receipt, policy, observation) -> Result<(), AdmissionError>`

## Required invariants

- known credential, key, cache, build-output and denied-system classes are denied by default
- unknown load-bearing policy/observation fields fail closed
- equal policy/observation inputs produce byte-identical decisions and receipts
- policy revision/fingerprint changes invalidate old receipts
- explicit overrides cannot bypass unconditional safety denies unless the policy schema authorizes it

## Typed failure surface

- `SOURCE_ADMISSION_DENIED`
- `SOURCE_KIND_UNSUPPORTED`
- `SOURCE_TOO_LARGE`
- `SENSITIVE_SOURCE_DENIED`
- `ADMISSION_POLICY_UNKNOWN_FIELD`
- `ADMISSION_RECEIPT_MISMATCH`

## Exit tests / evidence

- `default_sensitive_and_system_exclusions`
- `unknown_load_bearing_field_fails_closed`
- `deterministic_decision_and_receipt`
- `policy_revision_invalidates_receipt`
- `unconditional_deny_cannot_be_implicitly_overridden`
- `evaluator_performs_no_io`

## Suggested internal modules

```text
search-source-admission/src/
  policy.rs
  observation.rs
  rules.rs
  decision.rs
  receipt.rs
  fixtures.rs
  error.rs
```

## Size / split

- Initial `src/` target: **≤ 4,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Do not create one crate per rule. Split only for an independently qualified classifier/provider.
