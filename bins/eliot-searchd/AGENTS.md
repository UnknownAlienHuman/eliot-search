# Agent contract — eliot-searchd

The daemon owns composition in `bins/eliot-searchd/`. Use the
[root instructions](../../AGENTS.md) and [implementation guide](../../docs/implementation/README.md)
for a named owner and immediate-consumer cutover; unrelated package edits remain out of scope.

The [current local architecture](../../docs/architecture/README.md) supersedes old mandatory migration,
six-domain/secret-lifecycle and delegated-client composition. Do not fill old ports with synthetic
observations to bypass them; remove the obsolete requirement with its real producer/consumer.

## Ownership

- Connect real owners through direct calls or small private vendor/OS adapters.
- Order data-root acquisition, startup, provider admission, readiness, drain and shutdown.
- Keep real OS/root identity, lifecycle and bounded failures at effect boundaries.
- Connect the existing reader, revision and preparation code. Change their APIs together when removing
  obsolete domains, grants or intermediate receipts; package boundaries are not permanent constraints.

## Keep behavior in its existing owner

Retire the general control-migration platform for the local baseline. Unsupported old roots remain
untouched; neither legacy readers nor export implementation precede the new application. Revision
read/write belongs to one actual store; preparation reuses `search-materializer`. Use the concrete OS
credential owner only for secrets an implemented integration actually needs.
Do not copy those state machines into command handlers or create a second composition root.
Concrete store/vendor handles stay out of query/lifecycle public APIs. No hidden fallback.

[FUNCTIONS.md](FUNCTIONS.md) records implemented command limits and composition obligations.
A feature, compiled constructor or healthy process does not establish an installed handler or readiness.
When adding an external dependency, inspect its pinned API and exercise the actual integration.
Remove duplicate mechanisms before moving files; package line counts do not require another crate.
