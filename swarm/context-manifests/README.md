# Materialized writer contexts

Canonical layout:

```text
swarm/context-manifests/<package>/<context_record_sha256>.toml
```

`context_record_sha256` is the externally recorded SHA-256 of the complete committed manifest file. The
manifest's `signature.record_sha256` is a different value: the signed-payload digest over exact bytes
before the signature table.

The integration owner materializes a draft source list and exact registry fragments at one immutable base
commit, publishes one immutable writer-visible context artifact and records every source/snippet Git blob,
exact-byte digest and normalized UTF-8/LF digest. A later context change creates a new manifest and a new
or superseding ticket; an acknowledged context is never amended.

Use `swarm/CONTEXT_MANIFEST_TEMPLATE.md` and `swarm/schemas/context-manifest-v1.toml`. This directory
currently contains no materialized context.
