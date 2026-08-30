# Writer lease rendering guide

This guide is a human review view of `swarm/schemas/writer-lease-v1.toml`. The machine schema is
normative. The integration owner creates the immutable lease only after a valid assignment ticket and
materialized context exist.

Canonical record path:

```text
swarm/leases/<package>/<lease_id>.toml
```

## Identity

- `identity.lease_id`
- `identity.operation_id`
- `identity.package`
- `identity.stage`

## Ticket and context

- `ticket.ref`
- `ticket.sha256`
- `context.manifest_ref`
- `context.artifact_sha256`

All refs and digests are exact and must match the assignment ticket.

## Actors

- `actors.writer`
- `actors.reviewer`
- `actors.issuer`

Writer and reviewer are distinct and match the ticket. The issuer is the integration owner.

## Scope

- `scope.base_commit`
- `scope.branch_or_worktree`
- `scope.write_scope`
- `scope.feature_profile`
- `dependencies[]`

The lease grants only the exact package write scope at one immutable base commit. It cannot authorize
root/shared changes or dependency implementation access.

## Lifecycle

- `lifecycle.issued_at`
- `lifecycle.initial_state = LEASED`
- `lifecycle.automatic_expiry = false`
- `lifecycle.previous_active_lease_check = NONE_ACTIVE_VERIFIED`

A second active non-superseded lease for the same package is rejected. Wall-clock time never silently
expires or renews a lease.

The writer acknowledgement is not embedded into or inferred from the lease. It is a separate append-only
record:

```text
swarm/leases/<package>/events/<event_id>.toml
event.kind = ACKNOWLEDGED
event.reason_code = WRITER_ACKNOWLEDGED
```

Only after that event passes exact readback may implementation begin. Submission, revocation and
supersession also use append-only lifecycle events.

## Signature

- `signature.record_sha256`
- `signature.integration_signature_ref`

The embedded digest is the signed-payload digest. Complete-file identity is external.

## Non-claims

A lease cannot accept a package, select a provider/artifact, advance launch state, accept a gate or emit a
wave receipt.
