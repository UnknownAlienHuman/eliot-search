# search-source-records

Transitional schema/codec leaf containing relocated legacy DIRECT, root-catalog, currentness, batch and
provenance records. It owns no I/O or current source truth.

The new application needs a small typed current root/source/revision model in the source/control owner.
`PublishedView` owns immutable query bindings. Legacy registry batches, historical currentness receipts,
portfolio histories and source-root catalogue bytes are not baseline formats to preserve.

Move useful current-format value types beside their actual producer/consumer. Keep old codecs read-only
only while an explicit diagnostic consumes them, then delete this package and the registry/history
reexports. Do not create new records here merely to avoid changing real callers.

Unsupported old roots remain untouched. No source bodies, Qdrant data, access authority or readiness
claims belong in this package.
