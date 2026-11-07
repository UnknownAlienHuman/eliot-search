# search-control-redb

Plain root metadata, guarded current source/preparation state and fixed-generation publication metadata share
control/control.redb through [vendored redb 4.5.0](../../vendor/redb/ELIOT_VENDOR.md).
Native RO format/empty probes retain persistent-savepoint safety. Publication stores captured source metadata,
compact object/range/point reference pages and evidence, then commits a short guarded pointer switch.
Publication/recovery validates the complete graph; ordinary search pins the committed snapshot and validates
returned evidence plus retained objects. [FUNCTIONS.md](FUNCTIONS.md) owns the concrete APIs and boundaries.

Durable File/Tree admissions share source_state; native read-only capture defaults to no admissions on legacy
roots. Explicit PATH refreshes only its admission; bare index reloads their unique union. Confirmed absence
uses guarded current-inventory removal; ignored presence does not. Saved generation evidence, admission/kind
and immutable objects survive membership removal. Unchanged-key/ObjectRef remains a logical no-op. Unresolved old builds retain their complete
header/rows in retired obligations while a fresh successor may proceed; cleanup cannot erase those obligations.

Index retains one ControlWriter/Database across source/preparation and staging/upload, borrowing short durable
transactions. It explicitly closes before read-only acceptance, final pointer CAS and views, including failure
paths. A failed borrowed commit is not accepted by reading the live handle.
[The owning rule](FUNCTIONS.md#operation-scoped-control-writer) distinguishes database and transaction lifetime.

Source/Prepared immutable objects and external Qdrant effects belong to their existing owners; no writer
transaction spans their I/O. Legacy journals, authority/DEK APIs and exclusive fixtures remain removed.
NeedsRepair is distinct from corruption/unsupported format. The guarded writable-open capability may call
recover_plain_control: validate a recovered owned copy, recheck and recover/clean-close/inspect the original.
It never resets format metadata or copies candidate bytes back. Read-only opens remain non-repairing;
unsupported/corrupt originals are refused unchanged.
