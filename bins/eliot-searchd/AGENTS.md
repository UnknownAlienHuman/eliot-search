# Working on the daemon

Read [FUNCTIONS.md](FUNCTIONS.md), the current delivery stage and only affected owner recipes.
The current daemon binary dispatches four plain-root operator commands through search-runtime-owner and
search-control-redb. Source acquisition, immutable retention, preparation, Qdrant and framed queries are later work.

Trace invocation → concrete owner → immediate consumer. Move callers and physically delete displaced code
and exclusive fixtures together. Do not restore old command, feature or credential routes behind compatibility
wrappers. Preserve user data and unpublished #598 work.

The [root recipe](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md) owns exclusion, initialization,
opening and explicit incomplete-init discard. The daemon supplies the original deadline/cancellation and reports
observed state. Future source/query composition follows the
[implementation lock order](../../docs/implementation/README.md#lifetime-and-lock-order).

After production changes, run minimal locked Rust 1.98 check/Clippy for affected consumers including the actual
eliot-searchd binary. Final native root/control cases qualify only their executed scope; helper probes and
library checks do not establish an installed baseline or a source-to-Qdrant product.
