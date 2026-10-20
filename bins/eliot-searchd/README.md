# eliot-searchd

Composition root for the local Search application.

## Current executable truth

Production dispatch currently supports bounded one-shot scans:

```text
--scan-stdin QUERY
--scan-stdin-ascii-insensitive QUERY
--scan-file QUERY FILE
--scan-file-ascii-insensitive QUERY FILE
```

These scan current bytes only. They do not retain revisions, publish Qdrant points, survive restart or serve
the final local protocol.

Persistent mutation/read, source-root, initialization and serving command families currently return
`LEGACY_EFFECTS_UNAVAILABLE` in production or exist only under `#[cfg(test)]`. The help text still advertises
some of them. `--self-test` also contains a stale hard-coded `persistent_direct_store_available=true`; do not
use it as readiness evidence. [#264](https://github.com/UnknownAlienHuman/eliot-search/issues/264) owns the
connected replacement.

## Target ownership

The daemon constructs and owns the concrete application:

```text
current-format root + one process guard
→ current redb/object/source owners
→ one preparation path
→ one Qdrant client/runtime
→ publication and current-view owner
→ direct Search | Exact | Continue | Expand handlers
→ one bounded local dispatcher
```

It owns startup, factual readiness, request admission, cancellation/deadlines, shutdown and cleanup. It does
not own source policy meaning, query ranking rules, immutable-object algorithms or a second copy of product
state.

Read [AGENTS.md](AGENTS.md), [FUNCTIONS.md](FUNCTIONS.md), the
[architecture](../../docs/architecture/README.md) and
[implementation guide](../../docs/implementation/README.md). Historical DIRECT/sealed/proxy/migration
commands and their fixtures are deletion inputs, not supported product surfaces.
