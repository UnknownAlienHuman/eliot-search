# Working on search-qdrant-bridge

Follow the [root guidance](../../../AGENTS.md),
[architecture](../../../docs/architecture/README.md) and
[implementation](../../../docs/implementation/README.md). [FUNCTIONS.md](FUNCTIONS.md) maps the
current source, native APIs and remaining integration work.

Use the existing pinned `qdrant-client`, private vendor conversion and reusable connection/runtime.
Send bounded native `Document` inputs for `qdrant/bm25`, with matching ingestion/query text options.
Translate the selected published scope into retrieval filters and explicit IDF corpus parameters.
Use native indexed-branch fusion/grouping where useful; source bytes and evidence remain with their
existing owners.

The S9/S11 table describes current shared types. Simplify membership/profile/payload identity in the
shared owner, planner and bridge together. Index fields actually used for filtering/grouping; retain
one schema owner. Adapt precomputed-vector inputs for native BM25 rather than encoding it twice to
satisfy an old expected digest.

Use the actual configured endpoint, credentials and chosen server settings. Process startup belongs
to the supervisor. Check the capabilities/schema the selected path uses; artifact/profile receipts
and executed qualification reports are not runtime credentials.

For the local one-node, one-replica model, one publisher serializes dependent mutations. Require
completed, visible writes with `wait=true` before publication; queue acknowledgement is insufficient.
Strong ordering is not a universal floor for this model. Choose strict-mode cost limits deliberately.
Keep batches bounded and resolve uncertain writes through the existing publication owner before
advancing dependent work. Cancellation after possible effects does not imply rollback.

Trace producers and consumers when changing an API, coordinate active shared files and remove replaced
codecs/wrappers. Verify actual scope, native text options, stored results and write recovery after
integration. Historical source-shape diagnostics, compatibility receipts and line quotas do not
require preserving obsolete code.
