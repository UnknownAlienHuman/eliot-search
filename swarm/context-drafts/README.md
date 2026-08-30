# Non-claimable writer-context drafts

These records define exact source files and registry selectors for an issuance-time writer context. They
are not themselves mounted to a writer and contain no base commit or materialized artifact digest.

Canonical draft layout:

```text
swarm/context-drafts/<stage>/<package>.toml
```

At ticket issuance, the integration owner resolves the draft against one exact base commit and produces:

```text
swarm/context-manifests/<package>/<context_record_sha256>.toml
```

`context_record_sha256` is the external SHA-256 of the complete committed context-manifest file. It is
not the embedded signed-payload digest and is never calculated from a moving branch.

The context materializer extracts exact registry records, reads exact files, records per-source Git blob,
exact-byte and normalized UTF-8/LF identities, and emits one immutable writer-visible artifact in declared
order. A changed source byte, selector, accepted handoff, order or base commit creates a different context.

Drafts are non-claimable, unmaterialized and may not contain the architecture master, another package's
source tree, mutable dependency branches, source bodies or secrets.
