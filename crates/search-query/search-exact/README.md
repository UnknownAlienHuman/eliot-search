# search-exact

**C20 — Exact verification plane.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Compile and execute bounded exact scans against a frozen authoritative denominator and produce truthful proof reports.

## Owns

- ExactScanPlan compilation
- literal, safe regex, qualified-symbol and structural predicates
- frozen SourceRevision denominator
- execution completeness accounting
- negative-proof report semantics

## Must not own

- using indexed top-k as denominator
- semantic absence claims
- unbounded backtracking regex
- silently changing scope or revision during execution

- **Delivery wave:** W6 / P12
- **Soft source-line target:** 8,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
