# ELIOT Search configuration

Configuration supplies values consumed by the running application. Start with the
[implementation guide](../docs/implementation/README.md) and
[config source map](../crates/search-config/FUNCTIONS.md).

## Current problem

The current library/daemon stack is about **208 KiB of production Rust before tests**. It implements a
custom TOML grammar, dynamic registry, section ownership/security floors, provenance, canonical fingerprints,
diff/action planning, receipt-gated activation and readiness acceptance records.

Startup still rejects a valid non-default `query.limit`: defaults and candidate are compared, `ApplyLive` is
planned and no synthetic `LiveApply` receipt is supplied. Most registered values are not retained as typed
settings by their actual owners. Readiness additionally requires caller booleans such as
`search_accepted`/`indexed_accepted`.

This platform is a removal target, not a base for another reconfiguration layer.

## Current implementation direction

```text
one args_os invocation
+ one bounded TOML file
+ selected vars_os/CLI overrides
→ one typed DaemonSettings
→ validate actual fields
→ construct actual owners/handlers
→ retain running settings for status/new requests
```

Use the locked `toml` + Serde decoder and `deny_unknown_fields`. Bound file bytes before decoding. Keep paths
as OS values. Only documented fields with real consumers are accepted.

Selected settings include the actual root/source, storage, Qdrant, lexical/query and chosen optional-profile
values. Generic `[secrets]`, control migration epochs, qualification/gate receipts and planned subsystem fields
are not current configuration.

## Changes while running

A runtime update exists only when a concrete owner implements it:

- a query default may change for future requests;
- source selection/policy runs actual reconcile/publication or requires restart;
- root/storage/Qdrant endpoint/process settings require restart;
- lexical/model/schema changes use a new publication generation;
- unsupported changes fail without changing current settings.

No generic action DAG, config journal, completed-operation history or second recovery engine.

## Status and readiness

Status serializes selected redacted running settings and actual component state. It does not rebuild a generic
snapshot, emit a configuration fingerprint or require acceptance receipts. Search is ready when its actual
root/control/object/Qdrant/view/handler dependencies are ready. An unavailable optional component does not
block lexical Search.

## Historical material

[sections.toml](sections.toml), [section packets](sections/), the old example and W5–W10 files contain
historical proposals. They do not define supported fields. Update or delete them with the typed settings
cutover; do not preserve them through compatibility adapters.
