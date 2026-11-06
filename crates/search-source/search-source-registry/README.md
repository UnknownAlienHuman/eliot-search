# Current source-row values

This package owns only typed values crossing acquisition, object storage, and control. It contains no database, journal, replay protocol, history, receipts, admission policy, or in-memory registry.

CurrentSourceRecord binds the persisted root namespace and random SourceId to a native locator, verified NativeFileKey, immutable ObjectRef, and non-reset row and registration revisions. Locator bytes are an index key, not source identity. ObjectRef is content identity, not source identity.

The durable rows and atomic capture/commit operations belong to search-control-redb. Old record, history, provenance, admission, and reconciliation packages were removed with their disconnected callers. The work register keeps future reconciliation in R1 after the connected query path.
