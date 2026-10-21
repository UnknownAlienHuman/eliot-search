# Working on search-lexical

Read the [architecture](../../docs/architecture/README.md), [implementation](../../docs/implementation/README.md),
local [source/behavior map](FUNCTIONS.md) and [query intent](../search-query/README.md).

Connect actual preparation/query consumers to native Qdrant BM25 with explicit document/query preprocessing.
Reuse useful bounded Unicode identifier splitting, not the old numerical encoders, corpus statistics,
qualification records or legacy daemon index. Move a trivial remainder into its real consumer.

Do not claim that appending a whole identifier preserves it through the word tokenizer. Exact spelling/name
behavior remains independent of lexical top-k. Do not call a native scoped query O(top-k): the pinned donor
collects its filtered IDF population. The local map supplies concrete source evidence and required examples.

No lexical-provider trait, second encoder, IDF cache or new framework is selected by these findings. Keep
source coordinates and integrity with their actual owners. After code run minimal pinned compiler/Clippy for
changed real consumers; assess actual relevance and scoped-query costs before claims. Delete obsolete
vector/receipt/byte-shape fixtures with their implementations, not the useful behavioral examples.
