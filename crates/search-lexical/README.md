# search-lexical

**Cell C11 — Lexical Encoder.**

Deterministic text-to-sparse-vector encoding behind an explicit port.

- **Owns:** immutable lexical profiles; tokenization and identifier expansion semantics; weighting
  parameters pinned by fixture; golden document and query vectors; collision policy.
- **Must not own:** an inverted index or any searchable corpus.

No implicit language, stopword or stemming default is permitted. Exactly one lexical provider path is
selected per collection generation; switching providers is a migration, not a runtime choice.
