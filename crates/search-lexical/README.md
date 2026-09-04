# search-lexical

**C11 — deterministic lexical analyzer and sparse encoder.**

**Status:** implemented pure lexical analysis plus qualified sparse document/query encoding. No index or corpus store is owned by this crate.

Implemented behavior:

- bounded Unicode alphanumeric tokenization with exact original byte offsets;
- explicit case, underscore, minimum-length, stop-word and position-gap policy;
- deterministic term statistics;
- stable seeded term-to-index mapping;
- reject-or-measure collision policy with a finite rate threshold;
- raw, logarithmic and BM25 local TF weighting;
- binary, raw and logarithmic query TF weighting;
- explicit `None`, Qdrant-delegated, or frozen-local IDF mode;
- structural prevention of double IDF;
- sorted unique finite sparse vectors;
- exact profile/qualification/statistics/fingerprint receipts;
- no I/O, index mutation, provider selection, or hidden fallback.

A profile is unusable until `validate_sparse_profile` receives a matching accepted qualification receipt. Any tokenizer, mapping, weighting, IDF, collision, artifact or fixture change requires a different profile fingerprint and collection generation.

- **Delivery wave:** W3 / P06
- **Agent instructions:** [AGENTS.md](AGENTS.md)
