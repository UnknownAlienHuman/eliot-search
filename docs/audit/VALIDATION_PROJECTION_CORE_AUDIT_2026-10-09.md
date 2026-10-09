# Candidate validation and result projection core audit — 2026-10-09

**Repository:** `UnknownAlienHuman/eliot-search`  
**Audited source:** `4d25b060dfd6b7e5b60de13b065daa91f412b85d`  
**Scope:** `search-candidate-validator`, `search-result-projector` and their direct source/output contracts  
**Product qualification performed:** none

## Result

The validator/projector packages contain useful source-readback, gap, coverage, contract and disclosure mechanics. They are not yet an accepted evidence boundary. The validator compares two caller/port-provided expected structures without hashing the returned bytes, while the projector accepts caller-assembled handles, coverage, receipts and public evidence metadata.

The accepted sequence is now:

```text
#290 → #278 → #291 executor/ranking evidence
+ #256/#258/#267/#272/#274 source/index/access owners
→ #293 owner-derived validation with exact byte hashing
→ #294 owner-bound exact-ranked projection/coverage/handle/continuation
→ #279 live product pipeline
→ #280/#281 public recipes
→ #264 qualification
```

PR #133 is closed as packet-only. It is not an implementation branch.

---

## Findings

### F103 — CRITICAL — Nomination and validation context are caller-constructed authority

`CandidateNomination` publicly carries request/plan/leg, point/membership/collection/epoch/profile/source/unit/range and expected digest fields. `ValidationContext` separately carries allowed memberships, shadowed units, candidate ordinal/limit and request/live security state.

Two agreeing caller-created structures can bypass derivation from the canonical plan, indexed contract and immutable source view.

**Owner:** #293.

### F104 — CRITICAL — Returned source bytes are never hashed

`SourceReadback` returns bytes plus content/unit/excerpt digests. `reopen_and_verify` compares the digest fields to the expected fields but never hashes the returned bytes.

A buggy or hostile readback port can return arbitrary bytes of the expected length and echo the expected digests.

**Owner:** #293 through #237/#267.

### F105 — CRITICAL — Profile, residency, coordinate and assurance evidence is self-asserted

Readback includes ordinary fields/Booleans for profile, residency authorization, exact coordinate mapping and assurance. These values are accepted by equality or enum ordering rather than typed owner-issued evidence.

**Owner:** #293.

### F106 — HIGH — Per-call budgets bypass the aggregate request ledger

`candidate_ordinal`, `max_candidates` and `max_read_bytes` are supplied independently on each validation call. Repeated calls can reset the ordinal/read ceiling rather than consuming the one #291 request ledger.

**Owner:** #293.

### F107 — CRITICAL — Malformed backend evidence can be downgraded to one candidate gap

Wrong request/plan/membership/collection/profile/epoch/point/population evidence frequently becomes `ValidationOutcome::Gap`. Such evidence can indicate a malformed or contaminated whole leg and must not be repaired by dropping one hit and refilling from the same leg.

**Owner:** #293.

### F108 — HIGH — Candidate identity remains leg-dependent and has a fallback collision path

Validated candidate ID is formatted from `leg_id + point_id`. If formatting fails, code falls back to a profile-derived ID. The same point in different legs cannot merge and multiple candidates can share the fallback.

**Owner:** #278/#293.

### F109 — HIGH — Refill/coverage decision is count-only

`material_coverage_change` considers only before/after/target counts, a refill count and contamination Boolean. It does not know whether loss changed ranking, source/evidence-role quotas, exact denominator or required coverage.

**Owner:** #293/#294.

### F110 — CRITICAL — Public result metadata and envelope are caller-assembled

`CandidatePublicMetadata` and `CandidateSetEnvelope` let callers supply handle, evidence role, assurance, freshness, ranking trace, reason codes, receipt refs, plan/result fence, coverage and continuation.

Structural contract validation does not prove these fields came from one execution/source/security state.

**Owner:** #294.

### F111 — CRITICAL — Emission permit check is incomplete and stale

Projection checks only the permit checkpoint kind and membership. It does not compare source revision, exact range/unit/representation, binding/grant/owner/view/purge/disclosure or perform a fresh live check at actual output.

**Owner:** #294 through #274.

### F112 — CRITICAL — Handle binding is optional, weak and unused by projection

The existing helper compares only membership/revision/unit and is not called by `project_candidate_set`. It does not verify exact range/anchor/profile/residency/authority or the canonical handle-store record.

**Owner:** #294 through #283.

### F113 — CRITICAL — Projection omissions do not change coverage or continuation state

Count and byte-limit omissions are returned separately while caller-provided coverage and continuation are left unchanged. A result can claim complete coverage after omissions, and the continuation can fail to represent the exact remaining ordered set.

**Owner:** #294.

### F114 — HIGH — Projector re-sorts by raw `f32`

Projection orders candidates by `source_backed.raw_score.partial_cmp(...).unwrap_or(Equal)`, bypassing #278 exact fused score and full immutable tie key. It may compare raw scores from incompatible legs/populations.

**Owner:** #294 after #278.

### F115 — HIGH — Output budget does not measure output and input is sorted before ceiling

The “result byte” decision sums private validated source slice bytes rather than actual serialized public result/frame bytes. The package also accepts an arbitrary `Vec` and sorts every entry before enforcing candidate count.

**Owner:** #294 through #291.

### F116 — HIGH — Public evidence metadata can be elevated

Caller-selected assurance, freshness, evidence role, ranking trace and receipt refs can exceed what source/profile/execution evidence supports.

**Owner:** #294.

---

## Required design boundaries

### Validation

```text
#291 one-use terminal nominations and request ledger
+ #290 exact leg/profile/population
+ #256/#258 exact point/payload/vector evidence
+ #272 immutable membership/source/representation/unit truth
+ #267 exact immutable readback
+ #274 live checkpoint
→ #293 private ValidatedCandidateEvidence | Gap | LegContaminated
```

Returned bytes are hashed; owner evidence defines expected identity/profile/range. The backend cannot choose source truth.

### Projection

```text
#291 terminal coverage/remaining ledger
+ #278 exact ranked evidence
+ #293 validated source evidence
+ #274 live checkpoint
+ #283 exact range-bound handle
+ #285 exact remaining continuation
→ #294 prepared public result/output batch
```

All public fields are derived inside the owner. Every omission changes coverage and, when applicable, the exact continuation working set.

---

## Useful existing mechanics to preserve

- explicit precheck/readback/emission phases;
- immutable source revision rather than current path read;
- exact range-length check;
- explicit candidate gaps versus contamination vocabulary;
- assurance floor concept;
- public output excludes raw source bytes and paths;
- bounded reason-code/list contract types;
- explicit projection omissions.

Preserve these while replacing caller-shaped authority and digest/coverage shortcuts.

---

## Required serialization

```text
#290 → #278 → #291
+ #256/#258/#267/#272/#274
→ #293

#274 + #278 + #283 + #285 + #291 + #293
→ #294

#293 + #294
→ #279
→ #280/#281
→ #264 qualification
```

Agents must not reimplement validation/projection inside daemon recipe-host worktrees.

---

## GitHub reconciliation performed

- created #293 for owner-derived source validation and exact byte hashing;
- created #294 for owner-bound exact-ranked public projection;
- corrected #279/#280/#281/#264 dependencies;
- closed PR #133 as `[SUPERSEDED][PACKET-ONLY]`;
- recorded F103–F116 on coordinator #97.

---

## Evidence boundary

This is static source analysis. It did not:

- modify Rust source or dependencies;
- execute Cargo, Clippy or tests;
- run source stores, Qdrant, native provider or public recipe processes;
- prove a deployed exploit;
- qualify or enable source validation/projection/output.

Every implementation issue requires locked Rust 1.98 checks, strict Clippy and focused nonzero fixtures after code.