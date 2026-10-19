# Historical visible-epoch transaction

This file documented the schema-3 `VisibleEpochCommit` built on the generic journal, seven guard axes,
publication receipts, shadow records and compatibility schemas. Those bytes and state machines are not a
current compatibility obligation.

The replacement commits one typed `CurrentPublishedView` after the publication owner has completed and
read back the exact Qdrant effects described by its pending artifact. The transaction compares the expected
current view/pending operation, writes the new view and clears or advances only that pending state. It does
not copy point bodies/vectors into redb and does not append a permanent success receipt.

A view contains the route/generation/profile and manifest references needed by queries. A process-local
atomic pointer publishes the committed view after redb commit. Old guards retain old evidence until they
drain; cleanup follows their lifetime.

See [FUNCTIONS.md](FUNCTIONS.md) and
[`search-publication`](../search-index-qdrant/search-publication/README.md). Remove the old visibility,
shadow, guard and receipt codecs after callers move.
