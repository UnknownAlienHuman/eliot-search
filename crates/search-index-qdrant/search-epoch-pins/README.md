# search-epoch-pins → published-view guards

The useful responsibility is small: keep the exact published view used by a query alive until that query
and its result window finish.

Current source uses an `Arc<Mutex<_>>` registry and RAII guards, but also exposes raw route/epoch pins,
arbitrary route acquisition, caller-time continuation renewal and sampled reclamation watermarks. Those
APIs permit a new old-view pin after a deletion check and retain completed binding history.

The replacement owner holds one atomic current `PublishedView`. A query can only `pin_current()` and gets a
private RAII guard. A continuation receives another guard only from its still-live query guard using the
existing transfer pattern. After publication swaps the pointer, no new old-view guard can appear; the last
drop schedules cleanup.

Query windows are finite and process-local. Completion/expiry removes the record and drops the guard;
restart expires them and the client may repeat the query. No durable query lease, renewable old-view pin or
public reclaim permit.

[FUNCTIONS.md](FUNCTIONS.md) · [Implementation](../../../docs/implementation/README.md) ·
[Agent notes](AGENTS.md)
