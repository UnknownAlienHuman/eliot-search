# Access compiler core audit — 2026-10-09

**Repository:** `UnknownAlienHuman/eliot-search`  
**Audited source:** `e76ffb6b9f9675ae5b931d1e4223bebd3cbb69e7`  
**Scope:** `crates/search-query/search-access` and its direct pre-retrieval composition contract  
**Product qualification performed:** none

## Result

`search-access` contains substantial grant, scope, route, eligibility and restrictive-mutation logic. It is not an empty scaffold. It is also not yet a safe authority boundary: the public API lets ordinary callers construct many values described as authoritative, while the pre-retrieval compiler drops or ignores several load-bearing grant/profile constraints.

The accepted sequence is now:

```text
#287 owner-verified grant + exact #272 scope + effective ceilings
→ #288 canonical non-forgeable scope/route/overlap/eligibility compiler
→ #274 owner-issued live checkpoints + durable restrictive mutation
→ #275/#276/#279/#264 consumers
```

PR #117 is closed as packet-only. It is not an implementation branch.

---

## Findings

### F81 — CRITICAL — Grant authentication decisions are caller Booleans

`GrantValidationContext` exposes ordinary public fields:

```text
signature_verified
pairing_verified
nonce_accepted
```

`validate_grant` accepts an all-true caller object as proof of signature, pairing transcript and nonce/replay-ledger verification.

**Owner:** #287.

### F82 — CRITICAL — Grant-authorized scope is supplied separately by the caller

`GrantClaims` does not bind an exact immutable membership/scope revision. `PreRetrievalRequest` accepts an independent `grant_scope: BTreeSet<SourceMembershipId>`, and `intersect_scope` treats it as the authorization ceiling.

A caller can supply any set containing the requested memberships unless a composition layer independently prevents it.

**Owner:** #287 with #272.

### F83 — HIGH — Budget class is never authorized

`GrantClaims.allowed_budget_classes` is required only to be nonempty. Pre-retrieval compilation has no selected budget-class input and never tests membership in that set.

**Owner:** #287.

### F84 — HIGH — Grant byte ceilings disappear after validation

`max_source_read_bytes` and `max_result_bytes` are checked for nonzero values, but `PreRetrievalPlan` retains only a small `GrantFence`, scope, legs, predicates and permit. Effective grant ceilings are not carried into planner, executor, source readback, output, handle or continuation bindings.

**Owner:** #287.

### F85 — CRITICAL — Accepted access authority is publicly constructible

The package exposes public-field structures for access bindings, authoritative snapshots, authorized scopes, indexed routes, eligibility plans, safe legs, overlap proofs, live security state, permits and the final pre-retrieval plan.

A downstream caller can assemble matching-looking values without invoking the canonical compiler or reading owner state.

**Owner:** #288.

### F86 — CRITICAL — Overlap proof is forgeable and incompletely checked

`compile_safe_legs` groups multiple memberships into one scoring/IDF population when a caller-supplied proof matches only:

```text
route
membership set
access snapshot generation
```

It does not verify proof digest, `profile_digest`, complete population identity, access/scoring partitions, lineage/equivalence policy or current security/shadow/purge state. `profile_digest` is currently ignored.

A fabricated proof can combine unrelated populations and let denied or foreign material affect IDF/rank/count behavior.

**Owner:** #288.

### F87 — CRITICAL — Different generation domains are compared as one `u64`

`RequestSecurityFence.planned_generation` is populated from `AuthoritativeAccessSnapshot.generation`. `recheck_live_access` compares it numerically with `LiveSecurityState.generation`.

The source does not prove that access-snapshot and live-security counters share one sequence/domain. A numerically larger unrelated generation can appear fresh, while a valid independent generation can appear stale.

**Owner:** #288.

### F88 — HIGH — Snapshot and proof digests are supplied, not recomputed

`AuthoritativeAccessSnapshot.snapshot_digest`, `LiveSecurityState.snapshot_digest` and `OverlapFreeRouteProof.proof_digest` are ordinary fields. Package validation checks selected shapes/counts but does not recompute canonical digest/profile identity from owner-issued data.

**Owner:** #288 through #237/#272/#274.

### F89 — HIGH — Eligibility identity uses a package-local mixer

`derive_predicate_digest` fans a custom four-lane FNV-like mixer into a 32-byte `EligibilityPlanDigest`. It is not generated from the canonical #237/#258 eligibility contract and can drift from actual #262 filter semantics.

**Owner:** #288.

### F90 — HIGH — Grant and scope collections lack package-level finite admission limits

Recipes, modalities, budget classes and membership sets are public `BTreeSet`s. The package does not apply a closed accepted item/byte ceiling before these collections exist. A public caller can allocate excessive state before validation.

**Owner:** #287/#288.

---

## Required design boundaries

### Grant compiler

```text
signed grant artifact + issuer/profile
+ trusted installation/incarnation/BindingId
+ pairing transcript readback
+ nonce/replay-ledger readback
+ revocation/time observation
+ exact #272 scope revision/population
+ finite recipe/modality/budget/resource ceilings
→ opaque ValidatedGrant
```

There is no independent caller `grant_scope` and no all-true verification context.

### Scope/eligibility compiler

```text
#287 ValidatedGrant
+ #272 exact immutable scope
+ #258 indexed contract/profile
+ #262 route/generation/schema/epoch observation
+ #274 live security checkpoint
→ opaque CompiledAccessPlan
```

The plan carries exact effective budgets and one canonical population used consistently for retrieval, `idf.corpus`, count, facet/group, validation, trace and ranking.

### Overlap grouping

Without a complete owner-issued proof, memberships remain independent scoring populations. A proof must bind scope, route, profile, partitions, lineage/equivalence policy, security/shadow/purge generations and canonical digest. A matching membership set alone is never sufficient.

---

## Useful existing mechanics to preserve

- grant identity, expiry and revocation vocabulary;
- exact membership key versus embedded-ID consistency checks;
- fail-closed unknown/inactive membership behavior;
- independent singleton legs when no overlap proof is available;
- retrieval/IDF predicate parity concept;
- restrictive mutation ordering: block, commit/readback, publish, invalidate dependents, acknowledge;
- fail-closed pending security mutation recovery.

Preserve these invariants while replacing caller-shaped authority and custom digest/generation semantics.

---

## Required serialization

```text
#237 + #235 + #272
→ #287

#237 + #258 + #262 + #272 + #287
→ #288

#266/#267/#269/#272/#261/#262/#287/#288
→ #274

#274 + package-core handle/continuation owners
→ #275/#276/#279
→ #264 qualification
```

Agents must not repair grant/scope/proof compiler defects inside daemon call-site migrations.

---

## GitHub reconciliation performed

- created #287 for owner-verified grants, exact scope and effective ceilings;
- created #288 for canonical non-forgeable scopes/routes/overlap proofs/eligibility plans;
- added package-core prerequisites to #274;
- closed PR #117 as `[SUPERSEDED][PACKET-ONLY]`;
- recorded F81–F90 and the selected graph on #97.

---

## Evidence boundary

This is static source analysis. It did not:

- change Rust source or dependencies;
- run Cargo, Clippy or tests;
- verify a live grant issuer/nonce ledger/security journal;
- execute Qdrant or native Windows/provider paths;
- prove a deployed exploit;
- qualify or enable access/retrieval/output.

Every implementation issue requires locked Rust 1.98 checks, strict Clippy and focused nonzero fixtures after code.