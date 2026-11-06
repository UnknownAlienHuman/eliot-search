# redb 4.5.0 source provenance

This directory contains the published crates.io `redb` 4.5.0 source, with one native read-only API addition.

- Upstream repository: <https://github.com/cberner/redb>
- Published archive: <https://static.crates.io/crates/redb/redb-4.5.0.crate>
- Archive SHA-256: `8b317ef681e18f6091efbfab532eaf924a135c492d9d0299d0fbfae07e92a39b`
- Upstream Git commit, preserved in `.cargo_vcs_info.json`: `96ce31fe5cafcf735425a02f1781487a4287e2de`
- Licenses: unchanged `LICENSE-APACHE` and `LICENSE-MIT`.

All 81 published files were checked byte-for-byte against the checksum-verified archive before modification.
The 80 non-lock files are retained, including the normalized and original manifests, README, build script,
documentation, examples and tests. The published `Cargo.lock` is intentionally omitted: workspace lock
integration belongs to the verifier. The registry's `.cargo-ok` cache marker is not published source and is
not included.

## Exact source change

Only `src/db.rs` differs from the retained published files; the other 79 files are unchanged.
[ELIOT_VENDOR.patch](ELIOT_VENDOR.patch) is the complete diff against the published `src/db.rs`, with portable
relative paths and zero context. From the verified upstream source directory, apply it with
`git apply --unidiff-zero ELIOT_VENDOR.patch` to reproduce the modified file. This provenance note and the
patch are the only added files.

`ReadOnlyDatabase::has_persistent_savepoints() -> Result<bool, StorageError>` reads the existing typed
`persistent_savepoints` system table. Any row returns true, including a savepoint whose earlier user tables
are absent from the current catalogue. An absent or empty table returns false; metadata and storage errors
propagate. The query reads at most the first savepoint entry and creates no writable database, transaction,
repair, alternate store, or database-format parser.

The constructor retains the system root and concurrency mode. The accessor supports the default
`ExclusiveWriter` mode, whose file lock excludes writers throughout the read-only handle's lifetime.
The captured system root therefore matches the user-table snapshot read by CONTROL from that handle.
Other concurrency modes return `StorageError::Unsupported`; they do not silently combine different snapshots.
No file-format or persisted schema change is made.

CONTROL pins this exact version through its direct relative path dependency. This also selects the modified
API when CONTROL is consumed from an external workspace. The vendor package is excluded from the enclosing
workspace's automatic path-dependency membership.

Source review and rustfmt parsing do not establish runtime qualification. The verifier must run the real
savepoint/data-table/delete-current-table discard regression, confirm original database bytes remain unchanged,
and retain the legitimate fresh/preformat/formatted-empty cases.
