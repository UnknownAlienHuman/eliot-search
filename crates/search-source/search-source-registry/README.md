# Current source-row values

This package owns typed source/ObjectRef values and PreparationSettings shared by CONTROL, unitizer, object storage and the actual daemon/CLI. It contains no database, journal, replay protocol, history, receipts, admission policy or in-memory registry.

CurrentSourceRecord binds the persisted root namespace and random SourceId to a native locator, verified NativeFileKey, immutable ObjectRef, and non-reset row and registration revisions. Locator bytes are an index key, not source identity. ObjectRef is content identity, not source identity.

Durable rows and capture/CAS belong to search-control-redb; encoding belongs to the unitizer. See [FUNCTIONS.md](FUNCTIONS.md) for kind/settings bounds. Old record, history, provenance, admission and reconciliation packages were removed with their disconnected callers; future reconciliation remains R1.
