# search-safe-reader

**C06 — Safe Reader.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Acquire stable exact bytes from an already admitted source without executing content or escaping the admitted root.

## Owns

- final-handle root containment
- stable metadata/digest read protocol
- no-execute filesystem and Git-object acquisition
- byte-length and encoding observations

## Must not own

- source-admission policy
- root, identity or membership state
- parsing/materialization
- durable byte retention

- **Delivery wave:** W2 / P03
- **Soft source-line target:** 6,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
