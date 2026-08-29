# search-safe-reader

**C06 — Stable no-execute reader.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Acquire exact source bytes without executing content, escaping admitted roots or mislabeling unstable files.

## Owns

- final-handle root containment
- stable before/after metadata checks
- bounded retry and digest verification
- Git-object reads with hooks/prompts disabled
- encoding and byte-length observations

## Must not own

- parsing/materialization policy
- running hooks, macros, builds or remote resources
- following unadmitted reparse/symlink escapes
- retaining bytes durably

- **Delivery wave:** W2 / P03
- **Soft source-line target:** 7,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
