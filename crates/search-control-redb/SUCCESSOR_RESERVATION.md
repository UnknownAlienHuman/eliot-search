# Historical publication successor reservation

The previous API reserved a successor by validating a committed predecessor, snapshot publisher,
publication receipt history and schema-3 intent slot. That ceremony is superseded.

A new publication is prepared from the exact current `PublishedView`. The publication owner writes one
bounded operation artifact, then compare/sets the single `PendingPublication` against the expected current
view and absence of another pending operation. Epoch/generation allocation is owned by that operation.

No permanent predecessor receipt scan, intent-ID reuse search, snapshot-publisher authority or separate
successor-reservation record is required. If a commit acknowledgement is uncertain, reopen the current and
pending typed rows. If the operation already exists with the same input, continue it; conflicting state
fails.

See [PUBLICATION_INTENTS.md](PUBLICATION_INTENTS.md), [FUNCTIONS.md](FUNCTIONS.md) and the
[publication owner](../search-index-qdrant/search-publication/README.md). Remove the old successor API and
its exclusive fixtures with the journal platform.
