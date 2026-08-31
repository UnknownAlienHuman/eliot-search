# W3 non-claimable indexed-product agent drafts

**Status:** structural preparation only. W3 remains blocked until accepted `G1`, accepted `W2_G1`, exact qualified Qdrant artifact/client/schema evidence and package dependency handoffs.

The W3 lexical/index stage contains nine package assignments:

```text
A: search-lexical
   search-point-identity
   search-qdrant-supervisor
   search-qdrant-bridge
   search-epoch-pins

B: search-projection-planner
   search-index-reclaimer

C: search-publication

D: eliot-searchd (W3 re-entry)
```

Machine registry:

```text
swarm/w3-agent-packets.toml
```

Drafts:

```text
swarm/ticket-drafts/w3/<package>.toml
swarm/context-drafts/w3/<package>.toml
```

Every context is one bounded writer-visible artifact, at most sixteen static files, exactly three machine registry fragments and only immutable accepted dependency handoff records. Architecture Part I, dependency implementation source and prior W1/W2 implementation packets are excluded.

W3 qualification remains external and fail-closed:

```text
server artifact:      UNSELECTED / UNQUALIFIED
client revision:      UNSELECTED
collection schema:    NOT_ACCEPTED
mandatory probes:     NOT_EXECUTED
independent review:   ABSENT
indexed mode:         DISABLED
```

The daemon re-entry context follows [`W3_DAEMON_REENTRY.md`](W3_DAEMON_REENTRY.md): the W2 context is replaced by the accepted W2 daemon API/handoff, accepted `W2_G1` receipt and exact W3 library handoffs. A responding Qdrant process or collection never proves ownership, qualification, publication visibility or indexed readiness.

A ticket draft is not an issued ticket. A context draft is not a materialized context. Neither creates a lease, selects/downloads an artifact, enables indexed mode, accepts G2 evidence or emits a W3 receipt.

The valid progression remains:

```text
accepted G1 + W2_G1
→ exact Qdrant artifact/client/schema/probe qualification where applicable
→ materialize exact W3 package context
→ issue immutable assignment ticket
→ issue writer lease
→ exact writer acknowledgement
→ package-only implementation
→ submission
→ independent review
→ package handoff
→ separate W3 receipt
```

One writer owns one Cargo package. Parallelism is only between different packages after exact dependency and qualification receipts are accepted.
