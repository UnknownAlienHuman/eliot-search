# search-control-redb

Bounded technical control state, never a searchable corpus.

`PersistentControlJournal` is the concrete disk-backed adapter using pinned redb 2.6.3 without Python
features. It implements atomic record/receipt transactions, exact-input replay protection, unknown
commit recovery, read-only snapshots, current-receipt publication and explicit owner-epoch handoff.
Vendor types remain private. The previous `ControlJournal` reference model and its public API are
preserved in `src/reference.rs`; it is not a fallback for an unavailable disk backend.

The primary daemon has not yet migrated its existing file-based control state to this adapter.
Native file/root admission, migration, cancellation/deadline composition, pruning and full P02
qualification remain unfinished. The added tests have not been run in the authoring environment.

See [disk format, integration boundaries and verification](../../docs/runtime/CONTROL_REDB.md),
[function contract](FUNCTIONS.md) and [agent instructions](AGENTS.md).

```sh
cargo +1.98.0 test -p search-control-redb --lib --locked
```
