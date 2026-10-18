# Agent contract — eliot-searchd

The daemon owns composition in `bins/eliot-searchd/`. Use the
[root instructions](../../AGENTS.md) and [execution protocol](../../docs/product/EXECUTION_PROTOCOL.md)
for a named owner and immediate-consumer cutover; unrelated package edits remain out of scope.

The [current local architecture](../../docs/architecture/README.md) supersedes old mandatory migration,
six-domain/secret-lifecycle and delegated-client composition. Do not fill old ports with synthetic
observations to bypass them; remove the obsolete requirement with its real producer/consumer.

## Ownership

- Construct private concrete adapters and wire vendor-neutral owner ports.
- Order data-root acquisition, startup, provider admission, readiness, drain and shutdown.
- Supply actual qualified platform observations and stable, content-free integration reasons.
- Keep baseline DIRECT source admission/registry/identity, safe-reader, revision-store and materializer
  boundaries intact; their meaning does not depend on a historical wave or review receipt.

## Keep behavior in its existing owner

Retire the general control-migration platform for the local baseline. Keep any specifically required
retained-data export in its existing owner. Revision read/write belongs to one actual store;
preparation storage to `search-materializer`; secret protection to `search-os-secrets`.
Do not copy those state machines into command handlers or create a second composition root.
Concrete store/vendor handles stay out of query/lifecycle public APIs. No hidden fallback.

[FUNCTIONS.md](FUNCTIONS.md) records implemented command limits and composition obligations.
A feature, compiled constructor or healthy process does not establish an installed handler or readiness.
New external artifacts need the relevant exact qualification; test-report tokens and historical package/wave registries are not runtime authority. Review mixed responsibilities and remove duplicate mechanisms before moving
files; package line counts do not require another crate.
