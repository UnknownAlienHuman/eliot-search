# search-exact

**C20 — Exact verification plane.**

**Status:** `SOURCE_PRESENT`; literal, proof and exact-spine source exists, while the complete authoritative denominator/readback product path and installed qualification remain incomplete. See [central package status](../../../docs/product/PACKAGE_STATUS.toml). Current owner: #131.

Compile and execute bounded exact scans against a frozen authoritative denominator through vendor-neutral inventory/readback ports.

## Owns

The literal primitive uses one private Aho-Corasick 1.1.5 contiguous NFA for
case-sensitive and ASCII-insensitive scans, with Standard semantics and no prefilter.
State persists across borrowed chunks and overlapping matches. Full input validation
precedes construction; truncation occurs only at the first extra match. Donor build/start
failure returns the content-free `LITERAL_MATCHER_BUILD_FAILED` code without fallback.

Matching traverses bytes linearly and visits each supplied chunk once; auxiliary
storage is O(query bytes + returned matches). No measured performance claim follows.
All seven existing literal fixtures remain AUTHORED_NOT_RUN under #215. Multi-pattern,
regex and authoritative denominator integration remain #247/#248/#249; supplied-input
exhaustion alone never proves a complete corpus claim.

- exact predicate profiles and plans
- frozen denominator
- complete/incomplete execution accounting
- exact proof reports

## Must not own

- indexed top-k denominators
- semantic absence claims
- concrete source/storage/index adapters

- **Delivery wave:** W6 / P12
- **Soft source-line target:** 7,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
