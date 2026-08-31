# Append-only control-record supersessions

This directory is reserved for integration-owned immutable supersession receipts.

Canonical layout:

```text
swarm/supersessions/<record_kind>/<receipt_id>.toml
```

A supersession receipt binds one immutable historical control record to one independently valid
replacement record. It never edits or deletes the old record and never reuses its identity.

Allowed reasons are the closed `SupersessionReasonCode` values defined in
`swarm/schemas/types-v1.toml`. A supersession may redirect future consumers only through a separately
reviewed control-plane or launch change; it does not itself accept a package, gate or wave.

Use `swarm/schemas/supersession-receipt-v1.toml` and
`docs/handoff/TICKET_ISSUANCE_OPERATIONS.md`. This directory currently contains no supersession receipt.
