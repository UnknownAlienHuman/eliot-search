# Plain-root control metadata

The retained CONTROL implementation is `src/plain_control.rs`, exported by the minimal `src/lib.rs`.
Its sole dependency is redb 4.5.0, privately named `redb_plain`. The former journal, migration, authority,
DEK, reference implementations and their exclusive tests are physically removed.

The root owns `control/control.redb`. The current application format is the regular table
`plain_control_meta`, with the single row `"format_version" -> 1` using string keys and u64 values.
CONTROL stores no source bodies or retrieval index.

## Initialize under root exclusion

The caller holds root exclusion and the exact unfinished-init marker. Missing files use create-new semantics.
Existing zero-length files are opened without truncation and checked through their actual opened handle before
redb creation. Non-zero files are first opened read-only and classified within the empty-probe bound:

- No regular or multimap tables: clean preformat empty, eligible to finish initialization.
- Exactly the expected format table and row: formatted empty, accepted without rewriting it.
- Additional tables or metadata rows: nonempty, preserved and rejected for initialization.

A preformat retry opens the candidate file without truncation and repeats the read-only proof on that exact
OS file object. A retained handle survives the proof; the read-only database and transaction are explicitly
closed before the retained handle enters the writer. Its regular-file type and length are checked again.
There are no concurrent database handles sharing the duplicated handle's locks. The writer aborts any redb
repair request and checks that its transaction still has no regular or multimap tables before adding the format
row. It commits, explicitly closes, then reopens read-only to read back the real format marker before the root
publishes its manifest.

Cancellation or deadline expiry after vendor database creation or before the format transaction commits still
closes the database. A clean zero-table database remains eligible for an owned retry or discard. Unverifiable,
unsupported and repair-requiring databases are preserved; no legacy upgrade or generic recovery runs here.

## Read-only operations

`inspect_plain_control` opens the actual file through redb's maintained read-only backend and looks up the fixed
format table/key/version. It neither enumerates the catalogue nor imposes a whole-file size limit. Populated
current databases remain inspectable. This is format verification, not proof of every page's integrity.

`plain_control_is_empty` is only for unfinished initialization or discard under root exclusion. It accepts both
clean preformat empty and formatted empty states. Before catalogue enumeration it checks the same opened file
used by redb against a 64 MiB ceiling; larger files return `ProbeLimit`. Additional tables or metadata rows return
false. Required format metadata, when present as the sole table, must validate.

All operations retain the caller's original absolute deadline and cancellation flag and check them around I/O.
Typed redb errors map to bounded static codes; I/O failures are not classified as corruption. Native fresh-file
size, cancellation/retry cases and handle/lock behavior still require the designated runtime verifier's probes.
