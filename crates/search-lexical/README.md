# Lexical retrieval policy

The lexical Rust package and its local analyzer/alias/numerical/sparse encoders are retired. There is no
constants-only or forwarding Rust crate. [FUNCTIONS.md](FUNCTIONS.md) and [AGENTS.md](AGENTS.md) remain the
canonical lexical policy.

Publication supplies actual prepared unit text once. The concrete [bridge](../search-index-qdrant/search-qdrant-bridge/README.md)
privately applies the fixed Qdrant Code tokenizer, explicit BM25 text index and request-time k=1.2/b=0.75.
Returned nominations contain only routing metadata; retained objects and matching CONTROL evidence supply text.
Punctuation-sensitive Exact remains a separate retained-source operation.