# search-lexical

Qdrant native BM25 supplies numerical lexical ranking. Only a small code-term preparation helper may remain
in this package after its real preparation/query consumers replace the legacy local index and sparse engines.

The [source and behavior map](FUNCTIONS.md) owns the selected profile, identifier limitations, scoped-IDF cost,
caller cutover and behavior examples. Read it together with [query intent](../search-query/README.md) and the
[bridge](../search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md).

Identifier aliases improve lexical recall; word tokenization does not preserve exact punctuated spelling.
Exact literal/name operations have independent source-backed paths. Native execution does not make a query
independent of corpus size: filtered IDF performs population work in the selected Qdrant version.

Integration means one retained unit reaches native indexing/query and resolves to the correct evidence.
Useful ranking and large-corpus performance require additional observed results. No local numerical encoder,
provider registry, qualification service or compatibility wrapper is retained merely to satisfy old fixtures.
