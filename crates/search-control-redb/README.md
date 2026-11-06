# search-control-redb

Plain root metadata, guarded current source/preparation state and fixed-generation publication metadata share
control/control.redb through [vendored redb 4.5.0](../../vendor/redb/ELIOT_VENDOR.md).
Native RO format/empty probes retain persistent-savepoint safety. Publication stores actual source inputs,
upload text/pages and evidence, then commits a short guarded pointer switch and opens a complete validated
PublishedView. [FUNCTIONS.md](FUNCTIONS.md) owns the concrete APIs and boundaries.

Source/Prepared immutable objects and external Qdrant effects belong to their existing owners; no writer
transaction spans their I/O. Legacy journals, authority/DEK APIs and exclusive fixtures remain removed.
Unsupported database files remain untouched.
