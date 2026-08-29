# Agent contract — search-access

You own only `crates/search-query/search-access/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S8.3, S10.3, S14.4, S19, H10.3, H14, P08, P13.

## Mission

Validate grants, intersect scope with authoritative state and compile noninterfering pre-candidate access/scoring legs.

## Ownership

- grant validation and expiry/revocation checks
- server-authoritative scope intersection
- eligibility and IDF filter AST construction
- membership route deduplication and overlap proofs
- live deny/security mutation barrier semantics

## Forbidden ownership

- client-authored raw vendor filters or point IDs
- post-filter-only security
- mixing duplicate equivalent memberships in one IDF population
- granting authority from capability availability

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `validate_grant(claims, binding, live_state) -> Result<ValidatedGrant, AccessError>`
- `intersect_scope(request, grant, snapshot) -> Result<AuthorizedScope, AccessError>`
- `compile_safe_legs(scope, route_proofs) -> Result<Vec<SafeLeg>, AccessError>`
- `build_eligibility_filter(leg, fence) -> EligibilityAst`
- `revalidate_checkpoint(context, live_deny) -> Result<SecurityPermit, AccessError>`
- `invalidate_security_dependents(mutation) -> InvalidationSet`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `ACCESS_REVOKED`, `SECURITY_FAIL_CLOSED`, `INCOMPLETE_COVERAGE`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `unauthorized content never affects candidates, IDF, counts or traces`
- `retrieval and IDF filters are AST-equivalent`
- `duplicate memberships collapse or split into separate fused legs`
- `revocation during scoring discards and replans whole contaminated leg`
- `deny snapshot publish failure enters SECURITY_FAIL_CLOSED`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W4 / P08; hardened P13**
- Soft `src/` target: **8,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
