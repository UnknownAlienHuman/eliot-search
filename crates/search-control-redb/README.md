# search-control-redb

Plain root metadata, guarded current source/preparation state and fixed-generation publication metadata share
control/control.redb through [vendored redb 4.5.0](../../vendor/redb/ELIOT_VENDOR.md).
Native RO format/empty probes retain persistent-savepoint safety. Publication stores captured source metadata,
compact object/range/point reference pages and evidence, then commits a short guarded pointer switch.
Publication/recovery validates the complete graph; ordinary search pins the committed snapshot and validates
returned evidence plus retained objects. [FUNCTIONS.md](FUNCTIONS.md) owns the concrete APIs and boundaries.

Source/Prepared immutable objects and external Qdrant effects belong to their existing owners; no writer
transaction spans their I/O. Legacy journals, authority/DEK APIs and exclusive fixtures remain removed.
Unsupported database files remain untouched.
