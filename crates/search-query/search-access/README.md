# search-access

**C18 — Access compiler and live security.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Validate grants, intersect scope with authoritative state and compile noninterfering pre-candidate access/scoring legs.

## Owns

- grant validation and expiry/revocation checks
- server-authoritative scope intersection
- eligibility and IDF filter AST construction
- membership route deduplication and overlap proofs
- live deny/security mutation barrier semantics

## Must not own

- client-authored raw vendor filters or point IDs
- post-filter-only security
- mixing duplicate equivalent memberships in one IDF population
- granting authority from capability availability

- **Delivery wave:** W4 / P08; hardened P13
- **Soft source-line target:** 8,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
