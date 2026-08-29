# search-source-admission

**Security support for C03/C06 — Source admission policy.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Evaluate a versioned deny-by-default source-admission policy without reading source bytes or mutating
the source registry.

## Owns

- policy normalization and versioning
- path/metadata/format/sensitivity rule evaluation
- deterministic admission decisions and reason sets
- decision receipts bound to policy and observation digests

## Must not own

- filesystem/Git reads
- root registration or source membership state
- access authorization or ranking
- silent allow-on-unknown behavior

- **Delivery wave:** W2 / P03
- **Soft source-line target:** 4,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
