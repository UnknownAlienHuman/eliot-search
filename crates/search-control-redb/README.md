# search-control-redb

Plain local root control metadata, implemented by `plain_control` with directly path-pinned
[vendored redb 4.5.0](../../vendor/redb/ELIOT_VENDOR.md) as its sole dependency.
The public operations initialize an owned root's format marker, inspect the current format read-only, and
probe unfinished initialization for source-free emptiness, including persistent savepoint retention outside
the current table catalogue. [FUNCTIONS.md](FUNCTIONS.md) owns their behavior.

Legacy journals, migrations, authority/DEK APIs, reference implementations and exclusive fixtures are removed.
Unsupported old database files remain untouched. Source/prepared bodies and Qdrant state belong to their
existing owners outside this crate.
