# W4 non-claimable query-product agent drafts

**Status:** structural preparation only. W4 remains blocked until accepted `G1`, accepted `W3`, exact dependency handoffs and accepted query qualification evidence.

The W4 baseline query-product stage contains nine package assignments:

```text
A: search-access
   search-handles
   search-eval

B: search-query-planner
   search-candidate-validator

C: search-retrieval-executor
   search-result-projector
   search-continuation

D: eliot-searchd (W4 re-entry)
```

Machine registry:

```text
swarm/w4-agent-packets.toml
```

Drafts:

```text
swarm/ticket-drafts/w4/<package>.toml
swarm/context-drafts/w4/<package>.toml
```

Every context is one bounded writer-visible artifact, at most sixteen static files, exactly three machine registry fragments and only immutable accepted dependency handoff records. Architecture Part I, dependency implementation source and prior W1–W3 implementation packets are excluded.

Query qualification remains non-successful:

```text
baseline contract:       DESIGNED_NOT_EXECUTED
mandatory probes:        UNAVAILABLE
independent review:      ABSENT
query product:           DISABLED
```

The daemon re-entry context follows [`W4_DAEMON_REENTRY.md`](W4_DAEMON_REENTRY.md): the W3 daemon context is replaced by the accepted W3 daemon API/handoff, accepted W3 receipt and exact W4 library handoffs. A successful Qdrant request, plausible candidate or emitted snippet never proves source validity, authorization, complete coverage or query-product acceptance.

A draft is not a ticket. A context draft is not a materialized context. Neither creates a lease, enables query serving, accepts G2 or emits a W4 receipt.
