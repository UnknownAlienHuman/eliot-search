# Materialized writer contexts

Canonical layout:

```text
swarm/context-manifests/<package>/<context-digest>.toml
```

The integration owner materializes a draft source list and exact registry fragments at one base commit,
publishes one immutable writer-visible context artifact and records every source/snippet digest. A later
context change creates a new manifest and superseding ticket; an acknowledged context is never amended.

Use `swarm/CONTEXT_MANIFEST_TEMPLATE.md`. This directory currently contains no materialized context.
