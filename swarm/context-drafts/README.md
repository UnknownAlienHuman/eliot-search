# Non-claimable writer-context drafts

These records define exact source files and registry selectors for an issuance-time writer context. They
are not themselves mounted to a writer and contain no base commit or materialized artifact digest.

Canonical draft layout:

```text
swarm/context-drafts/<stage>/<package>.toml
```

At ticket issuance, the integration owner resolves the draft against one exact base commit and produces:

```text
swarm/context-manifests/<package>/<context-digest>.toml
```

The context materializer extracts exact registry records, reads exact files, records per-source SHA-256
and emits one immutable writer-visible artifact in declared order. A changed source byte, selector,
accepted handoff or order creates a different context.

Drafts are non-claimable, unmaterialized and may not contain the architecture master, another package's
source tree, mutable dependency branches, source bodies or secrets.
