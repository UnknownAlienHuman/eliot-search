# W2 non-claimable source-spine agent drafts

**Status:** structural preparation only. W2 remains blocked until accepted `G0` and `W1` receipts.

The W2 DIRECT source spine contains eight package assignments:

```text
A: search-source-admission
   search-source-identity
   search-safe-reader
   search-revision-store
   search-materializer
   search-unitizer

B: search-source-registry

C: eliot-searchd (W2 re-entry)
```

Group A packages may proceed in parallel only after every exact W1/foundation dependency handoff required
by their tickets is accepted. Group B starts only after accepted `search-source-admission` and
`search-source-identity` handoffs. Group C starts only after all seven W2 library handoffs plus the prior
accepted W1 daemon API/handoff and W1 receipt.

Machine registry:

```text
swarm/w2-agent-packets.toml
```

Drafts:

```text
swarm/ticket-drafts/w2/<package>.toml
swarm/context-drafts/w2/<package>.toml
```

Every context is one bounded writer-visible artifact, at most sixteen static files, exactly three machine
registry fragments and only immutable accepted dependency handoff records. Architecture Part I,
dependency implementation source and the prior W1 implementation packet are excluded.

The daemon re-entry context follows [`W2_DAEMON_REENTRY.md`](W2_DAEMON_REENTRY.md): the previous W1
writer context is replaced by accepted W1 API/evidence receipts and the new W2 source-spine context. W2
composition is DIRECT-only; Qdrant, indexed retrieval, query-product and later profiles remain absent.

A ticket draft is not an issued ticket. A context draft is not a materialized context. Neither creates a
lease, implementation authority, package acceptance, G1 evidence or a W2 receipt.

The valid progression remains:

```text
accepted G0 + W1
→ materialize exact W2 package context
→ issue immutable assignment ticket
→ issue writer lease
→ exact writer acknowledgement
→ package-only implementation
→ submission
→ independent review
→ package handoff
→ separate W2/G1 receipt
```

One writer owns one Cargo package. Parallelism is only between different packages after exact dependency
handoffs are accepted.
