# search-subject-resolver

**C21 — Subject resolution.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Resolve an entity under an explicit source view using a deterministic ladder and return ambiguity instead of guessing.

## Owns

- cursor/handle/qualified-name resolution ladder
- exact name and signature compatibility
- bounded SubjectAmbiguitySet
- resolution evidence and assurance

## Must not own

- normative selection among materially different definitions
- online repository discovery
- final comparison or ranking verdict

- **Delivery wave:** W6 / P11
- **Soft source-line target:** 6,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
