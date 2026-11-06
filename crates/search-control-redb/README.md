# search-control-redb

Plain local root control metadata, implemented by `plain_control` with redb 4.5.0 as its sole dependency.
The public operations initialize an owned root's format marker, inspect the current format read-only, and
probe unfinished initialization for source-free emptiness. [FUNCTIONS.md](FUNCTIONS.md) owns their behavior.

Legacy journals, migrations, authority/DEK APIs, reference implementations and exclusive fixtures are removed.
Unsupported old database files remain untouched. Source/prepared bodies and Qdrant state belong to their
existing owners outside this crate.
