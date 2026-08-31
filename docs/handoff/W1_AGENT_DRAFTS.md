# W1 non-claimable agent drafts

**Status:** structural preparation only. W1 remains blocked until accepted `G0` and `W0` receipts.

The W1 process/control shell contains seven Cargo packages and exactly one future writer per package:

```text
A: search-config
B: search-runtime-owner | search-os-secrets | search-control-redb | search-provider-protocol
C: eliot-searchd | eliot-search
```

Group B starts only after an accepted `search-config` handoff. `eliot-searchd` additionally requires all
four Group B library handoffs. The CLI requires accepted `search-config` and
`search-provider-protocol` handoffs.

Machine registry:

```text
swarm/w1-agent-packets.toml
```

Drafts:

```text
swarm/ticket-drafts/w1/<package>.toml
swarm/context-drafts/w1/<package>.toml
```

Every context is one bounded writer-visible artifact, at most sixteen static files, at most four exact
registry fragments, no architecture master and no dependency implementation source. Accepted dependency
handoffs are embedded by immutable record identity and digest only.

A draft is not a ticket. A context draft is not a materialized context. Neither creates a lease,
implementation authority, package acceptance, G1 evidence or a W1 receipt.

The valid progression remains:

```text
accepted G0 + W0
→ materialize exact W1 package context
→ issue immutable assignment ticket
→ issue writer lease
→ exact writer acknowledgement
→ package-only implementation
→ submission
→ independent review
→ package handoff
→ separate W1 receipt
```

One writer may advance through package-local milestones sequentially, but multiple milestone agents may
not write the same crate. Parallelism is only between different packages after all exact dependency
handoffs are accepted.
