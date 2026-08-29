# Wave receipts

This directory is reserved for integration-owned, append-only gate and launch-transition receipts.

Canonical layout:

```text
swarm/wave-receipts/<wave>/<receipt-digest>.toml
```

A receipt binds accepted package commits/API digests, registry/workspace graph identities, mandatory
gate evidence, unresolved state and the corresponding launch-state transition.

Rules:

- only the integration owner issues a receipt;
- every mandatory evidence ID in `swarm/gates.toml` appears with `PASS`, `FAIL` or `UNAVAILABLE`;
- a wave can be accepted only when its mandatory evidence is `PASS`;
- `UNAVAILABLE` remains explicit and cannot satisfy a runtime, security or performance requirement;
- no unresolved active package lease may remain at advancement;
- the receipt and launch-state transition are part of the same reviewed integration change;
- accepted receipts are immutable and corrections supersede by reference.

Canonical bytes follow `swarm/RECEIPT_CANONICALIZATION.md`.
