# search-safe-reader

Obtain stable bytes from a selected local source and pass them to retention without executing the
indexed material. Source policy, durable revisions and preparation have separate existing owners.

**Current source:** `safe_read`, `safe_read_with_cancel`, `safe_read_with_retries` and
`acquire_with_cancel_v2` share a same-handle kernel. Daemon filesystem adapters exist. The Git module
parses supplied decompressed loose-object bytes through an abstract backend; no real repository/pack
reader is connected. This is not a functioning Git ingestion backend.

**Target:** one verified byte handoff to the revision writer and a concrete local Git batch reader.
Controlled system Git is the chosen first implementation; gix is an alternative if standalone packaging
requires operation without Git. Neither path executes source hooks/filters or fetches missing content.

See [implementation](../../../docs/implementation/README.md) and [FUNCTIONS.md](FUNCTIONS.md) for
existing symbols, raw-object semantics and donor source.
