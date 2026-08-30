# Non-claimable writer-context drafts

These schema-v2 records define exact source files and registry selectors for an issuance-time writer
context. They are not themselves mounted to a writer and contain no selected base commit or materialized
output identity.

Canonical draft layout:

```text
swarm/context-drafts/<stage>/<package>.toml
```

A draft keeps the future manifest and artifact identities distinct and unresolved:

```text
materialized_context_manifest_ref
materialized_context_record_sha256
materialized_context_artifact_ref
materialized_context_artifact_sha256
```

At ticket issuance, the integration owner resolves the draft against one exact base commit and produces:

```text
swarm/context-manifests/<package>/<context_record_sha256>.toml
```

`context_record_sha256` is the external SHA-256 of the complete committed context-manifest file. It is
not the embedded signed-payload digest and is never calculated from a moving branch. The context artifact
has its own immutable ref and SHA-256.

The context materializer extracts exact registry records, reads exact files, records per-source Git blob,
exact-byte and normalized UTF-8/LF identities, and emits one immutable writer-visible artifact in declared
order. A changed source byte, selector, accepted handoff, order or base commit creates a different context.

## Source ceilings

`swarm/context-drafts/manifest.toml` is authoritative:

```text
ordinary package source files:       at most 16
search-contracts P00 exact-pack:     at most 24
registry fragments per context:      at most 6
accepted handoff slots per context:  at most 1
writer-visible artifacts:            exactly 1
```

The only source-count exception is `search-contracts`. It exists because one W0 foundation writer must
consume the manifest-closed P00 schema pack. The exception may include only the exact P00 manifest closure
plus fixed integration instructions and registry fragments; it cannot add the architecture master,
foreign source trees or unrelated stage packets. `search-domain`, `search-ports` and every W1+ package
remain under the ordinary sixteen-file ceiling.

Drafts are non-claimable, unmaterialized and may not contain the architecture master, another package's
source tree, mutable dependency branches, source bodies or secrets.
