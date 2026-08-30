# Ticket issuance conformance corpus

This directory contains the bounded, synthetic qualification contract for the integration-owned ticket
issuance control plane.

## Files

- [`manifest.toml`](manifest.toml) — exact paths, counts, authority nonclaims and evidence policy.
- [`baseline.toml`](baseline.toml) — locked operation, recovery, identity, context, lease and zero-state
  behavior.
- [`fixtures.toml`](fixtures.toml) — 52 synthetic fixture descriptors; no real record or actor assignment.
- [`probes.toml`](probes.toml) — 64 mandatory probes covering all failures and recovery dispositions.
- [`TICKET_ISSUANCE_QUALIFICATION.md`](TICKET_ISSUANCE_QUALIFICATION.md) — execution and acceptance
  contract.

Machine operation ownership is defined by
[`../../swarm/control-plane-operations.toml`](../../swarm/control-plane-operations.toml). Record fields and
canonical layouts remain owned by the control-plane schema/type registries.

## Manual structural validation

```powershell
pwsh -NoProfile -File tools/validate-ticket-issuance-conformance.ps1
pwsh -NoProfile -File tools/validate-ticket-issuance-conformance.ps1 -Json
```

A structural PASS proves only corpus closure. Every probe remains `UNAVAILABLE` until immutable raw output
and an independent reviewer receipt exist.

## Nonclaims

This corpus does not:

- materialize a context;
- assign writer or reviewer identities;
- issue a ticket or lease;
- record acknowledgement/submission/review;
- publish a package handoff;
- accept G0/W0;
- advance launch state.
