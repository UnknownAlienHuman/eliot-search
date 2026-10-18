# search-publication

Own one recoverable transition from a prepared change to one immutable `PublishedView`.

The current coordinator is process-local bookkeeping: its named methods accept receipts/observations but
do not write redb, call Qdrant or publish the daemon query snapshot. The daemon has a second Publisher.
These paths must become one owner, not two synchronized state machines.

Target sequence:

1. write ordered bounded operation/manifest pages;
2. persist a small redb pending record referencing them;
3. perform Qdrant full/narrow mutations and exact readback;
4. commit route/epoch/current manifest in redb;
5. atomically swap the current published-view pointer;
6. retain old manifests while view guards exist;
7. clean exact retired IDs or the whole old generation, then remove operation-only pages.

A pre-dispatch cancellation discards local work. A timeout after possible external effect remains the same
pending operation. No caller-created proof digest, process-local replay map or permanent completed history.

The published manifest owns exact source evidence bindings. Retrieval points may remain stable across views;
old pinned views keep their old revision/range. Publication does not mutate historical citation meaning.

[FUNCTIONS.md](FUNCTIONS.md) · [View guards](../search-epoch-pins/FUNCTIONS.md) ·
[Cleanup](../search-index-reclaimer/FUNCTIONS.md) · [Implementation](../../../docs/implementation/README.md)
