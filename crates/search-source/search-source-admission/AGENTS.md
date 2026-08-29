# Agent contract — search-source-admission

You own only `crates/search-source/search-source-admission/`. Do not edit another package, the root
workspace, shared contracts or architecture. Missing fields use the contract-change process.

The Architecture 8.4 master is not required for ordinary work. This is the package slice.
Traceability only: S7.7-S7.8, S16.5-S16.6, H3.3, P03.

## Mission

Produce one deterministic, versioned and auditable admission decision before source preparation.

## Ownership

- canonical `SourceAdmissionPolicy` normalization and fingerprinting
- deny-by-default evaluation of path class, metadata, source kind, size and sensitivity observations
- explicit allow/deny/needs-review decisions with ordered reason codes
- decision receipts bound to policy fingerprint and observation digest
- policy property tests and default exclusion fixtures

## Forbidden ownership

- opening files, following links, reading Git objects or retaining bytes
- registering roots, identities or memberships
- access authorization after admission
- content classification that requires reading source bodies in the baseline
- allowing an unknown load-bearing observation or policy field
- embedding user-specific absolute paths in reusable policy fixtures

## Allowed dependencies

`search-contracts`, `search-domain`. The evaluator must remain pure: no filesystem, clock, redb,
Qdrant, process or network dependency.

## Required logical surface

- `normalize_policy(policy) -> Result<CanonicalAdmissionPolicy, AdmissionError>`
- `evaluate_admission(policy, observation) -> AdmissionDecision`
- `decision_receipt(policy, observation, decision) -> AdmissionReceipt`
- `verify_receipt(receipt, policy, observation) -> Result<(), AdmissionError>`

## Failure surface

Relevant public reasons include `SOURCE_ADMISSION_DENIED`, `SOURCE_KIND_UNSUPPORTED`,
`SOURCE_TOO_LARGE`, `SENSITIVE_SOURCE_DENIED` and `ADMISSION_POLICY_UNKNOWN_FIELD`.

## Test seams and exit evidence

- `default policy denies known secret/system/cache/build-output classes`
- `unknown load-bearing field fails closed`
- `same policy and observation produce byte-identical receipt`
- `policy revision changes fingerprint and invalidates stale receipt`
- `allow rule cannot override an unconditional safety deny without explicit versioned rule`
- `evaluator performs no I/O and owns no mutable state`

## Size and split guard

- Delivery wave: **W2 / P03**
- Soft `src/` target: **4,500 lines**
- Hard review threshold: **10,000 hand-written Rust lines**
- Do not split one rule per crate. Split only if an independently replaceable classifier is admitted by ADR.

## Definition of done

The package is pure, deterministic, deny-by-default, fully fixture-tested and produces a receipt the
registry can persist without reimplementing policy.
