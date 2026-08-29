# search-lexical

**C11 — Deterministic lexical encoder.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Encode documents and queries into immutable sparse-vector profiles without owning an index.

## Owns

- code_v1 and text_neutral_v1 profile behavior
- tokenization, Unicode and identifier expansion
- document/query compatibility fixtures
- term-index/collision policy
- profile and fixture digests

## Must not own

- inverted-index or searchable corpus storage
- implicit English stopwords/stemming
- runtime fallback between providers
- using BM25 to prove exact identity or absence

- **Delivery wave:** W3 / P06
- **Soft source-line target:** 8,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
