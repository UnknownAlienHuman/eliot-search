<a id="working-on-search-qdrant-supervisor"></a>
# Working on the legacy Qdrant supervisor

Read [FUNCTIONS.md](FUNCTIONS.md). The baseline needs a bridge connection to the operator-started local server,
not this package. Do not implement launch receipts, artifact qualification, secret leases, quarantine/restart
state or a Windows containment platform merely because their types exist.

Before deletion, trace current source/feature/target callers and move any useful behavior to its real consumer.
Preserve shared OS/file helpers and user data. CLI-owned daemon cleanup is separate; never kill the external
Qdrant process. A future managed-server request needs a small concrete owner, not restoration of this framework.

Use the [working guide](../../../docs/implementation/WORKING_GUIDE.md#minimal-production-checks) for the affected
production closure. Package fixtures and old source findings do not prove current product reachability.
