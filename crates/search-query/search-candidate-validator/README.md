# search-candidate-validator — evidence lookup, not a proof owner

A ranked Qdrant hit becomes useful only after the pinned `PublishedView` maps its point ID to one exact
source revision, representation, occurrence and range, and the retained-source owner returns the bytes.

## Current source

`src/lib.rs` is about 24.5 KiB. It defines public nominations, validation contexts, expected readback
records, security/plan/profile/residency fields, per-call budgets, permits, refill classification and
validated-candidate identities.

The current `reopen_and_verify` compares digest fields supplied alongside `SourceReadback.bytes`; it does
not hash or otherwise establish a relation between those bytes and the expected values. Public-main daemon
integration uses fixed test readback, not the production retained-source owner.

## Target evidence resolution

```text
RankedHit(point_id, score, contributions)
+ pinned PublishedView
+ ResolvedScope
+ request deadline / remaining read-output budget
→ lookup ViewManifestEntry(point_id)
→ open exact retained object / prepared representation
→ verify the actual object under its storage integrity policy
→ validate range and coordinate mapping
→ return VerifiedEvidenceItem or explicit Gap
```

`VerifiedEvidenceItem` is a private value consumed immediately by result formatting:

```text
point identity and final rank/contributions
source / revision / representation / logical occurrence
exact range/location
verified excerpt bytes/text
freshness / supported assurance derived from the real source/preparation records
```

Do not copy expected digests into a readback response and compare them. The immutable-object reader should
verify the object content/address once according to its actual format; a range read then proves membership
in that verified object and checks bounds/coordinates. Do not rehash a multi-megabyte file separately for
every excerpt merely to create another receipt.

## Failure classification

- point ID absent from the pinned view, foreign membership/route/generation or contradictory point
  readback: backend/view inconsistency; fail or contaminate the query leg;
- manifest entry exists but retained revision/prepared artifact/range is unavailable or unsupported:
  candidate-local gap;
- cancellation, deadline or remaining read/output budget: incomplete result with explicit gap;
- restricted-session revocation: cancel the session/window through its real owner.

A rejected point never produces an excerpt. Candidate loss cannot make ranked top-k exhaustive.

## Cutover

Change executor output, manifest lookup, retained reader, result formatting and actual daemon handler
together. Remove public `CandidateNomination`, `ValidationContext`, expected digest/readback profiles,
per-call ordinal/budget resets, grant/security permits, residency booleans and leg-derived fallback IDs.

If the surviving lookup/read is small, fold it into query composition or the retained-source reader and
delete this package. [FUNCTIONS.md](FUNCTIONS.md) maps the current API. [AGENTS.md](AGENTS.md) governs work.
