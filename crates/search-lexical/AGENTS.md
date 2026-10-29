# Working on lexical retrieval

Read the [profile recipe](FUNCTIONS.md), then actual preparation and query consumers. Use its selected Code
text-index profile; do not restore the earlier word-tokenizer/sparse-inference design or local alias expansion.
The [bridge](../search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) alone translates vendor APIs.

Keep source coordinates and integrity with their owners. Physical-profile changes rebuild; query-time k/b does
not. Top-k limits returned nominations, not backend population work. Measure actual relevance and cost rather
than asserting them from tokenizer or cache behavior.

Remove local numerical encoders, statistics, qualification records and exclusive fixtures with callers. No
provider trait, IDF cache or second index is needed. After code, run minimal pinned check/Clippy for real consumers.
