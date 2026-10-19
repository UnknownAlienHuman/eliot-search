# Pending publication state

The old typed-intent design stored a long-lived publication state machine inside the generic control
journal and depended on operation receipts for loss detection. It is superseded.

The product keeps at most one small typed `PendingPublication`:

```text
operation artifact reference
expected current view
route / physical generation / profile
next effect, readback and cleanup cursor
finite limits and created/expiry metadata if needed
```

The immutable artifact contains the bounded manifest/change/input pages needed to resume actual Qdrant
work. redb contains no source bodies, vectors or full point batches. Restart reads the pending record and
artifact directly, observes Qdrant/current view state and resumes idempotently.

After successful `CurrentPublishedView` commit, operation-only cursor/checkpoint pages and the pending
record are removed. Old/new view manifest pages remain only while current, pending or live guards refer to
them. No terminal intent history, consumed-epoch receipt ledger or compatibility schema is retained.

See [FUNCTIONS.md](FUNCTIONS.md) and
[`search-publication`](../search-index-qdrant/search-publication/FUNCTIONS.md).
