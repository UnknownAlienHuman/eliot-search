# search-source-admission

**Security support for C03/C06 — Source admission policy.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Evaluate a versioned deny-by-default source-admission policy without reading source bodies or mutating registry state.

## Owns

- canonical policy normalization and fingerprinting
- path/metadata/format/sensitivity observation evaluation
- deterministic decisions, reasons and receipts
- default exclusion fixtures

## Must not own

- filesystem/Git reads
- root registration, identity or membership state
- post-admission access authority
- silent allow-on-unknown behavior

- **Delivery wave:** W2 / P03
- **Soft source-line target:** 4,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
