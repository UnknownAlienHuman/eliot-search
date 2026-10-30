# Working on indexing and publication

Use the [family map](README.md), current delivery stage and affected owner recipe. Do not preload every package.
Follow [local scope](../../docs/architecture/LOCAL_SCOPE.md): the configured backend is on this machine.

## Selected baseline

One bridge connects to the operator-started server. Do not construct the old supervisor, provision a key, spawn
Qdrant or add a backend-mode selector to make the first Search work. Connection and index readiness are distinct;
the [bridge](search-qdrant-bridge/FUNCTIONS.md) owns both cases and the exact dependency/schema cut.

## Publication order

Use [publication](search-publication/FUNCTIONS.md) for fixed input, bounded metadata commit and view installation.
Stage 2/3A uses fresh immutable generations. Do not implement Stage 3B identities/mutation before measured need.

## Lifetime and cleanup

Preserve exact old-view evidence, finite work and whole-generation cleanup. Never hold a redb writer or view
mutex over external I/O. Do not duplicate the publisher's recovery rules in another package.

## Optional processes/backends

Managed Qdrant and Edge are not baseline work or fallback routes. No new launcher, platform containment service,
secret lifecycle or generic backend trait. Recheck real callers before removing obsolete source and fixtures.
After code, use the [working guide](../../docs/implementation/WORKING_GUIDE.md#minimal-production-checks) for the
actual bridge/publication/query/daemon closure; a library-only or cfg-excluded check cannot qualify wiring.
