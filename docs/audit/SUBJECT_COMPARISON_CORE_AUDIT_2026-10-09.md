# Subject resolution and comparison core audit — 2026-10-09

**Repository:** `UnknownAlienHuman/eliot-search`  
**Audited source:** `a9acbe77ad39588bda42865033c4f8557b4dbebd`  
**Scope:** `search-subject-resolver` and `search-comparator`  
**Product qualification performed:** none

## Result

Both packages contain valuable deterministic semantics: a strict subject ladder, explicit ambiguity, evidence roles, configuration variants, non-normative comparison vocabulary and lineage collapse. Their current accepted inputs are nevertheless ordinary public structures containing caller-provided Booleans, digests, receipt refs, handles and completeness claims.

The accepted graph is now:

```text
#213/#277 + #272/#274/#293/#294
→ #296 owner-derived complete subject ladder
→ #232 orientation

#296 + #272/#274/#293/#294
→ #297 exact comparison corpus/lineage/configuration/absence evidence
→ #298 non-normative matrix/coverage/recommended reading/output
→ #281 evidence/orientation/comparison recipes
→ #264 qualification
```

---

## Findings

### F117 — CRITICAL — Subject request/context authority is caller-shaped

`SubjectRequest` and `ResolutionContext` publicly accept selector/context/owner/security digests plus `cancelled`, `scope_non_empty`, `observation_complete`, `access_permitted`, `purge_clear`, `view_current` and `owner_generation_current` Booleans.

**Owner:** #296.

### F118 — CRITICAL — Subject candidates are caller-assembled evidence

`SubjectCandidate` publicly accepts subject/handle, candidate/hypothesis/source/coordinate/context digests, assurance, portfolio priority, authorized/current/entity-compatible Booleans and equivalence/evidence receipt refs.

**Owner:** #296.

### F119 — CRITICAL — Caller can mark a ladder rung complete

A public `ResolutionStep::Complete` with zero omissions and an empty candidate list permits fall-through to weaker rungs or final `NotFound` without an owner-issued denominator/completeness proof.

**Owner:** #296.

### F120 — CRITICAL — Equivalence collapse checks only receipt presence

Candidates collapse under caller `hypothesis_digest` whenever `equivalence_receipt_ref.is_some()`. Receipt endpoints/profile/view/currentness are not verified.

**Owner:** #296.

### F121 — HIGH — Resolution receipt accepts arbitrary hashing

`issue_resolution_receipt` accepts a `blake3_256` callback and wraps its output as `Blake3Digest32`, bypassing #237.

**Owner:** #296.

### F122 — CRITICAL — Resolution and candidate set can contradict

Receipt issuance accepts a caller resolution and candidate vector separately. A resolved digest need not belong to the executed candidates; contradictory candidates can accompany `NotFound`, `ScopeEmpty` or `Incomplete`.

**Owner:** #296.

### F123 — CRITICAL — Live resolution revalidation is caller-shaped

`ResolutionLiveState` contains caller digests and access/purge/observation Booleans rather than #274 owner readback.

**Owner:** #296.

### F124 — HIGH — Ambiguity handles and ordering evidence are caller-provided

Ambiguity output copies candidate handles and orders representatives using caller assurance, portfolio priority, source and coordinate digests.

**Owner:** #296 through #294.

### F125 — CRITICAL — Comparison profile/portfolio completeness/currentness is caller-shaped

`ComparisonProfile`, `ComparisonRequest` and `ComparisonLiveState` accept profile qualification, inventory completeness/counts, context digests and access/purge/current Booleans without owner proof.

**Owner:** #297.

### F126 — CRITICAL — Comparable candidates/observations rely on caller trust flags

Candidates and observations carry exact-readback/authorized/current/entity-compatible Booleans, arbitrary handles, summaries, digests and receipt refs.

**Owner:** #297.

### F127 — CRITICAL — Weak analogue basis is admitted by receipt presence alone

Exact-name or lexical matching becomes comparable when `analogue_receipt_ref.is_some()`; endpoints/profile/entity/signature/currentness are not verified.

**Owner:** #297.

### F128 — CRITICAL — Public lineage receipts can union arbitrary lineages

`collapse_repository_lineages` trusts public relation/current/portfolio/receipt fields and does not verify lineage evidence through an owner.

**Owner:** #297.

### F129 — CRITICAL — Missing lineage evidence counts as independent

The union-find starts each lineage independently. No relation receipt has the same counting effect as an owner-proven `Independent` relation, inflating evidence when lineage observation is incomplete.

**Owner:** #297/#298.

### F130 — CRITICAL — Predicate relation kind is caller-issued

`PredicateRelationReceipt` lets callers state equivalent/mutually-exclusive/overlapping/unknown and a current Boolean. Conflict/variant classification accepts it after only context comparison.

**Owner:** #297.

### F131 — CRITICAL — Exact local absence is a caller set

`LocalBehaviorEvidence.exact_complete_axes` controls local-absence claims; accompanying absence receipt refs are not verified by comparison.

**Owner:** #297.

### F132 — HIGH — Signature and coverage digests accept arbitrary callbacks

Behavior signature and coverage functions accept arbitrary `blake3_256` callbacks and use saturating count conversions.

**Owner:** #297/#298 through #237.

### F133 — HIGH — Normative language is not structurally blocked

Only a request Boolean forbids normative verdict. Caller observation summaries, signature text, headers and descriptions can still say “best”, “correct”, “adopt” or equivalent and flow to public output.

**Owner:** #298.

### F134 — HIGH — Recommended reading and final behavior set are caller-assembled

Reading candidates provide handle, role, assurance, lineage/source/coordinate and authorization fields. Final assembly accepts separate caller header/local/candidates/signatures/matrix/reading without one live-bound request/portfolio/profile proof.

**Owner:** #298.

---

## Required boundaries

### Subject resolution

```text
versioned selector + exact scope/view/checkpoint
+ owner-complete rung denominators
+ #293 evidence + #294 handles
+ verified equivalence relation
→ atomic resolution + receipt
```

No caller marks a rung complete, supplies an accepted candidate or collapses hypotheses by receipt presence.

### Comparison input

```text
#296 exact local subject
+ #272 exact portfolio inventory
+ #293/#294 source-backed observations/handles
+ complete owner lineage/configuration relation evidence
+ exact per-axis absence denominator
→ private PreparedComparisonCorpus
```

Missing lineage/configuration/absence evidence remains unknown.

### Comparison output

```text
PreparedComparisonCorpus
+ #274 checkpoint
+ accepted profile and remaining budget
→ canonical non-normative matrix/coverage/reading/result/receipt
```

Every gap/truncation changes coverage and continuation state.

---

## GitHub reconciliation performed

- created #296 for owner-derived subject resolution;
- created #297 for exact portfolio/candidate/observation/lineage/configuration evidence;
- created #298 for matrix/coverage/non-normative reading/output;
- corrected #232/#281/#264 dependencies;
- recorded F117–F134 on coordinator #97.

---

## Evidence boundary

This is static source analysis. It did not modify Rust source/dependencies, run Cargo/Clippy/tests, exercise provider/Qdrant/Windows paths or qualify public recipes. Every implementation issue requires locked Rust 1.98 checks, strict Clippy and focused nonzero fixtures after code.